//! The write surface's contracts against the real schema, DB-backed through
//! `db::scratch` (skipped without `CATALYST_TEST_DATABASE_URL`); worlds and
//! wallets are uuid-scoped, so the tests share one database without seeing
//! one another. The name authority is a marketplace subgraph stub on the
//! loopback (the old tests seeded squid's `ens`/`nft` tables).

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use crate::auth::hash_bytes_v1;
use crate::chain_index::ChainIndex;
use crate::chain_index::graph::test_support::{Names, index_with_names};
use crate::comms::world_identity::{Parcel, ParcelSet, SceneId, WorldName};
use crate::content::forward;
use crate::policy::{Deploy, Permission, PersonalWorldsPolicy, Policy};
use crate::rooms;
use crate::shared::identity::Address;
use crate::shared::signed_fetch::ORIGINAL_PATH_HEADER;
use catalyst_storage::Storage;

use super::auth::test_support::{Wallet, signed_headers};
use super::deploy_form::test_support::multipart_body;
use super::store::{
    self, AccessSetting, AllowListEdit, AllowListEditOutcome, OrderDirection, SceneDeployment,
    WorldSettingsUpdate, WorldsListFilters, WorldsListOptions, WorldsOrderBy,
};
use super::{CoreState, Settings, SharedState, router};

const ADMIN_TOKEN: &str = "cg-worlds-admin";
const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

struct Fixture {
    pool: PgPool,
    storage: Arc<Storage>,
    state: SharedState,
    app: Router,
    names: Names,
    /// The name authority behind `names`; its 30 s memo is cleared by the
    /// tests that move a NAME.
    market: Arc<ChainIndex>,
}

/// Personal worlds are not published here: the policy's cap counts every
/// personal world in the shared scratch database, so a route test that
/// leaves one behind would fail `policy::tests`; the cap and the personal
/// owner rule are covered there.
/// `forward_to`: the publishes are queued for that (loopback) upstream.
async fn fixture_with(personal: PersonalWorldsPolicy, forward_to: Option<&str>) -> Option<Fixture> {
    let pool = crate::db::scratch::pool(8).await?;
    let (market, names) = index_with_names().await;
    let dir = std::env::temp_dir().join(format!(
        "catalyst-worlds-core-{}-{}",
        std::process::id(),
        Uuid::new_v4().simple()
    ));
    let storage = Arc::new(Storage::from_config(&dir).await.unwrap());
    let policy = Policy::new(pool.clone(), personal, Some(market.clone()));
    let forwarder = forward_to.map(|target| {
        forward::test_support::forwarder(
            pool.clone(),
            storage.clone(),
            super::forwarding::LANE,
            target,
            3,
        )
    });
    let state = Arc::new(
        CoreState::new(
            pool.clone(),
            storage.clone(),
            Arc::new(policy),
            Settings::for_tests(Some(ADMIN_TOKEN)),
            None,
        )
        .with_forwarder(forwarder),
    );
    let app = router(state.clone());
    Some(Fixture {
        pool,
        storage,
        state,
        app,
        names,
        market,
    })
}

async fn fixture() -> Option<Fixture> {
    fixture_with(PersonalWorldsPolicy::disabled(), None).await
}

fn world_name() -> WorldName {
    WorldName::parse(&format!(
        "w{}.dcl.eth",
        &Uuid::new_v4().simple().to_string()[..12]
    ))
    .unwrap()
}

struct Case {
    method: &'static str,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<(Vec<u8>, String)>,
}

impl Case {
    fn new(method: &'static str, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            headers: Vec::new(),
            body: None,
        }
    }
    async fn signed(self, w: &Wallet) -> Self {
        self.signed_meta(w, "{}").await
    }
    async fn signed_meta(self, w: &Wallet, metadata: &str) -> Self {
        let path = self.path.split('?').next().unwrap().to_string();
        self.signed_over(w, &path, metadata).await
    }
    /// Signed over `path`, whatever path the request is sent to.
    async fn signed_over(mut self, w: &Wallet, path: &str, metadata: &str) -> Self {
        let headers = signed_headers(w, self.method, path, metadata).await;
        for (k, v) in headers.iter() {
            self.headers
                .push((k.to_string(), v.to_str().unwrap().to_string()));
        }
        self
    }
    fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
    fn bearer(mut self, token: &str) -> Self {
        self.headers
            .push(("authorization".to_string(), format!("Bearer {token}")));
        self
    }
    fn json(mut self, v: &Value) -> Self {
        self.body = Some((
            serde_json::to_vec(v).unwrap(),
            "application/json".to_string(),
        ));
        self
    }
    fn body(mut self, bytes: Vec<u8>, content_type: &str) -> Self {
        self.body = Some((bytes, content_type.to_string()));
        self
    }
}

async fn hit(app: &Router, case: Case) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(case.method.to_uppercase().as_str())
        .uri(&case.path);
    for (k, v) in &case.headers {
        builder = builder.header(k, v);
    }
    let request = match case.body {
        Some((bytes, ct)) => builder
            .header(header::CONTENT_TYPE, ct)
            .body(Body::from(bytes))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()))
    };
    (status, value)
}

async fn expect(app: &Router, case: Case, status: u16) -> Value {
    let path = case.path.clone();
    let method = case.method;
    let (got, body) = hit(app, case).await;
    assert_eq!(got.as_u16(), status, "{method} {path}: {body}");
    body
}

struct Deployment {
    body: Vec<u8>,
    content_type: String,
    entity_id: String,
    thumb_hash: String,
}

async fn deployment_with(
    wallet: &Wallet,
    world: &WorldName,
    title: &str,
    base: &str,
    pointers: &[&str],
    nav_thumb: &str,
    thumb: &[u8],
) -> Deployment {
    let now = chrono::Utc::now().timestamp_millis();
    deployment_at(wallet, world, title, base, pointers, nav_thumb, thumb, now).await
}

#[allow(clippy::too_many_arguments)]
async fn deployment_at(
    wallet: &Wallet,
    world: &WorldName,
    title: &str,
    base: &str,
    pointers: &[&str],
    nav_thumb: &str,
    thumb: &[u8],
    timestamp: i64,
) -> Deployment {
    let thumb_hash = hash_bytes_v1(thumb);
    let entity = json!({
        "type": "scene",
        "timestamp": timestamp,
        "pointers": pointers,
        "content": [{ "file": nav_thumb, "hash": thumb_hash }],
        "metadata": {
            "display": { "title": title, "navmapThumbnail": nav_thumb },
            "worldConfiguration": { "name": world.display() },
            "scene": { "base": base, "parcels": pointers }
        }
    });
    let entity_bytes = serde_json::to_vec(&entity).unwrap();
    let entity_id = hash_bytes_v1(&entity_bytes);
    let chain = serde_json::to_string(&wallet.chain(&entity_id).await).unwrap();
    let (body, content_type) = multipart_body(&[
        ("entityId", None, entity_id.as_bytes()),
        ("authChain", None, chain.as_bytes()),
        ("entity.json", Some("entity.json"), &entity_bytes),
        ("thumb.png", Some("thumb.png"), thumb),
    ]);
    Deployment {
        body,
        content_type,
        entity_id,
        thumb_hash,
    }
}

async fn deployment(
    wallet: &Wallet,
    world: &WorldName,
    title: &str,
    base: &str,
    pointers: &[&str],
) -> Deployment {
    let thumb = format!("{title} thumb").into_bytes();
    deployment_with(wallet, world, title, base, pointers, "thumb.png", &thumb).await
}

async fn post_entity(app: &Router, d: &Deployment, status: u16) -> Value {
    expect(
        app,
        Case::new("post", "/entities").body(d.body.clone(), &d.content_type),
        status,
    )
    .await
}

async fn stored_owner(pool: &PgPool, world: &WorldName) -> Option<String> {
    sqlx::query_scalar("SELECT owner FROM worlds.worlds WHERE name = $1")
        .bind(world.key())
        .fetch_optional(pool)
        .await
        .unwrap()
        .flatten()
}

async fn deployed_entity_ids(pool: &PgPool, world: &WorldName) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT entity_id FROM worlds.world_scenes WHERE world_name = $1 ORDER BY entity_id",
    )
    .bind(world.key())
    .fetch_all(pool)
    .await
    .unwrap()
}

/// A parcel-scoped `deployment` grant, written the way the permissions write
/// surface writes it: one grant row plus one parcel row per parcel.
async fn grant_parcels(pool: &PgPool, world: &WorldName, address: &Address, parcels: &[&str]) {
    sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1) ON CONFLICT (name) DO NOTHING")
        .bind(world.key())
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO worlds.world_permissions (world_name, permission, address) VALUES ($1, 'deployment', $2)
         ON CONFLICT (world_name, permission, address) DO UPDATE SET updated_at = timezone('UTC', now())",
    )
    .bind(world.key())
    .bind(address)
    .execute(pool)
    .await
    .unwrap();
    for parcel in parcels {
        sqlx::query("INSERT INTO worlds.world_permission_parcels (world_name, permission, address, parcel) VALUES ($1, 'deployment', $2, $3) ON CONFLICT DO NOTHING")
            .bind(world.key())
            .bind(address)
            .bind(*parcel)
            .execute(pool)
            .await
            .unwrap();
    }
}

fn parcels(raw: &[&str]) -> ParcelSet {
    ParcelSet::parse(raw).unwrap()
}

fn scene_entity(
    world: &WorldName,
    title: &str,
    thumb: &str,
    base: &str,
    parcels: &[&str],
) -> Value {
    json!({
        "type": "scene",
        "timestamp": 1000,
        "pointers": parcels,
        "content": [{ "file": "thumb.png", "hash": thumb }],
        "metadata": {
            "display": { "title": title, "description": "A test world", "navmapThumbnail": "thumb.png" },
            "worldConfiguration": {
                "name": world.display(),
                "skyboxConfig": { "fixedTime": 36000 },
                "fixedAdapter": "offline:offline"
            },
            "scene": { "base": base, "parcels": parcels },
            "tags": ["art", "game"],
            "rating": "E"
        }
    })
}

/// A store-level deploy (the old `WorldsComponent::deploy_scene` call shape).
async fn deploy(
    f: &Fixture,
    world: &WorldName,
    owner: &Address,
    entity_id: &str,
    entity: &Value,
    footprint: &[&str],
    replacement: &Deploy,
) -> Result<(), super::http::ApiError> {
    store::deploy_scene(
        &f.pool,
        &f.storage,
        SceneDeployment {
            world,
            name_owner: Some(owner),
            entity_id,
            deployer: owner,
            deployment_auth_chain: &json!([{ "type": "SIGNER", "payload": owner.as_str() }]),
            entity,
            parcels: &parcels(footprint),
            size: 1,
            replacement,
            forward_to: None,
        },
    )
    .await
}

async fn entity_ids(f: &Fixture, world: &WorldName) -> Vec<String> {
    let mut ids: Vec<String> = store::list_scenes(&f.pool, world)
        .await
        .unwrap()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    ids.sort();
    ids
}

#[tokio::test]
async fn acl_deployer_cannot_take_over_an_existing_world_owner() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let name_owner = Wallet::random();
    let acl_deployer = Wallet::random();
    f.names.set(&name_owner.address, world.label());

    let d = deployment(&name_owner, &world, "Owner World", "0,0", &["0,0", "0,1"]).await;
    post_entity(&f.app, &d, 200).await;
    assert_eq!(
        stored_owner(&f.pool, &world).await.as_deref(),
        Some(name_owner.address.as_str())
    );

    let path = format!("/world/{}/permissions/deployment", world.display());
    let meta = json!({ "type": "allow-list", "wallets": [acl_deployer.display()] }).to_string();
    expect(
        &f.app,
        Case::new("post", &path)
            .signed_meta(&name_owner, &meta)
            .await,
        204,
    )
    .await;

    f.names.forget(&name_owner.address);

    let d = deployment(&acl_deployer, &world, "ACL World", "0,0", &["0,0", "0,1"]).await;
    post_entity(&f.app, &d, 200).await;
    assert_eq!(
        stored_owner(&f.pool, &world).await.as_deref(),
        Some(name_owner.address.as_str()),
        "an ACL deployer rewrote worlds.owner to {}",
        acl_deployer.address.as_str()
    );

    let permissions = expect(
        &f.app,
        Case::new("get", format!("/world/{}/permissions", world.display())),
        200,
    )
    .await;
    assert_eq!(
        permissions["owner"].as_str(),
        Some(name_owner.address.as_str())
    );
    assert_eq!(
        permissions["permissions"]["deployment"]["wallets"],
        json!([acl_deployer.address.as_str()])
    );

    expect(
        &f.app,
        Case::new("post", &path)
            .signed_meta(&acl_deployer, &meta)
            .await,
        403,
    )
    .await;
}

#[tokio::test]
async fn first_deploy_seeds_the_resolved_name_owner() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let name_owner = Wallet::random();
    f.names.set(&name_owner.address, world.label());
    let d = deployment(&name_owner, &world, "Seeded World", "0,0", &["0,0", "0,1"]).await;
    post_entity(&f.app, &d, 200).await;
    assert_eq!(
        stored_owner(&f.pool, &world).await.as_deref(),
        Some(name_owner.address.as_str())
    );
    // A stranger (no NAME) is refused with the old message, and nothing lands.
    let stranger = Wallet::random();
    let other = world_name();
    let d = deployment(&stranger, &other, "Nope", "0,0", &["0,0"]).await;
    let body = post_entity(&f.app, &d, 403).await;
    assert_eq!(
        body["errors"][0],
        format!(
            "The signer {} is not authorized to deploy to the world {}",
            stranger.address.as_str(),
            other.key()
        )
    );
    assert!(deployed_entity_ids(&f.pool, &other).await.is_empty());
}

/// A NAME transfer: the previous holder loses the publish and every owner
/// route at once (the authority is asked, `worlds.owner` is only its cache),
/// the new holder's publish rewrites the cache.
#[tokio::test]
async fn a_name_transfer_moves_ownership_on_the_next_publish() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let (old, new) = (Wallet::random(), Wallet::random());
    f.names.set(&old.address, world.label());
    post_entity(
        &f.app,
        &deployment(&old, &world, "Before", "0,0", &["0,0"]).await,
        200,
    )
    .await;
    assert_eq!(
        stored_owner(&f.pool, &world).await.as_deref(),
        Some(old.address.as_str())
    );

    f.names.forget(&old.address);
    f.names.set(&new.address, world.label());
    f.market.graph.names.clear();

    let body = post_entity(
        &f.app,
        &deployment(&old, &world, "Stale", "0,0", &["0,0"]).await,
        403,
    )
    .await;
    assert_eq!(
        body["errors"][0],
        format!(
            "The signer {} is not authorized to deploy to the world {}",
            old.address.as_str(),
            world.key()
        )
    );
    let path = format!("/world/{}/permissions/deployment", world.display());
    let meta = json!({ "type": "allow-list", "wallets": [] }).to_string();
    expect(
        &f.app,
        Case::new("post", &path).signed_meta(&old, &meta).await,
        403,
    )
    .await;
    assert_eq!(
        stored_owner(&f.pool, &world).await.as_deref(),
        Some(old.address.as_str()),
        "a refused publish rewrites nothing"
    );

    post_entity(
        &f.app,
        &deployment(&new, &world, "After", "0,0", &["0,0"]).await,
        200,
    )
    .await;
    assert_eq!(
        stored_owner(&f.pool, &world).await.as_deref(),
        Some(new.address.as_str())
    );
    expect(
        &f.app,
        Case::new("post", &path).signed_meta(&new, &meta).await,
        204,
    )
    .await;
    let permissions = expect(
        &f.app,
        Case::new("get", format!("/world/{}/permissions", world.display())),
        200,
    )
    .await;
    assert_eq!(permissions["owner"].as_str(), Some(new.address.as_str()));
}

/// A world this node only mirrored becomes local on its owner's publish: the
/// overlapping mirrored scene goes, the new scene is local and the world
/// row's `mirror_source` is cleared in the same transaction (the mirror then
/// refuses to write it, `worlds::mirror`); a stranger is refused as before.
#[tokio::test]
async fn an_owner_publish_converts_a_mirrored_world_to_local() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let (owner, stranger) = (Wallet::random(), Wallet::random());
    f.names.set(&owner.address, world.label());
    sqlx::query(
        "INSERT INTO worlds.worlds (name, mirror_source) VALUES ($1, 'https://upstream.example')",
    )
    .bind(world.key())
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source) \
         VALUES ($1, 'bafymirrored', '[]'::json, '{}'::jsonb, $2, ARRAY['0,0'], 1, 'https://upstream.example')",
    )
    .bind(world.key())
    .bind(crate::ZERO_ADDRESS)
    .execute(&f.pool)
    .await
    .unwrap();

    post_entity(
        &f.app,
        &deployment(&stranger, &world, "Nope", "0,0", &["0,0"]).await,
        403,
    )
    .await;
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT entity_id, mirror_source FROM worlds.world_scenes WHERE world_name = $1",
    )
    .bind(world.key())
    .fetch_all(&f.pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![(
            "bafymirrored".to_string(),
            Some("https://upstream.example".to_string())
        )]
    );

    post_entity(
        &f.app,
        &deployment(&owner, &world, "Mine now", "0,0", &["0,0"]).await,
        200,
    )
    .await;
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT entity_id, mirror_source FROM worlds.world_scenes WHERE world_name = $1",
    )
    .bind(world.key())
    .fetch_all(&f.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].1.is_none(),
        "the local scene replaced the mirrored one"
    );
    let (stored, mirror): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT owner, mirror_source FROM worlds.worlds WHERE name = $1")
            .bind(world.key())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(
        (stored.as_deref(), mirror),
        (Some(owner.address.as_str()), None),
        "the world row says local"
    );
}

/// The mirror never refreshes a local world, so the publish that converts a
/// mirrored world takes every mirrored scene with it, the ones away from its
/// parcels too: none is left behind, stale, beside the local scene.
#[tokio::test]
async fn a_local_publish_leaves_no_mirrored_scene_in_the_world() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let owner = Wallet::random();
    f.names.set(&owner.address, world.label());
    sqlx::query(
        "INSERT INTO worlds.worlds (name, mirror_source) VALUES ($1, 'https://upstream.example')",
    )
    .bind(world.key())
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source) \
         VALUES ($1, 'bafyover', '[]'::json, '{}'::jsonb, $2, ARRAY['0,0'], 1, 'https://upstream.example'), \
                ($1, 'bafyaway', '[]'::json, '{}'::jsonb, $2, ARRAY['5,5'], 1, 'https://upstream.example')",
    )
    .bind(world.key())
    .bind(crate::ZERO_ADDRESS)
    .execute(&f.pool)
    .await
    .unwrap();

    post_entity(
        &f.app,
        &deployment(&owner, &world, "Mine now", "0,0", &["0,0"]).await,
        200,
    )
    .await;
    let mirrored: Vec<String> = sqlx::query_scalar(
        "SELECT entity_id FROM worlds.world_scenes WHERE world_name = $1 AND mirror_source IS NOT NULL",
    )
    .bind(world.key())
    .fetch_all(&f.pool)
    .await
    .unwrap();
    assert_eq!(mirrored, Vec::<String>::new());
    let local: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM worlds.world_scenes WHERE world_name = $1 AND mirror_source IS NULL",
    )
    .bind(world.key())
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(local, 1);
}

#[tokio::test]
async fn a_parcel_scoped_grant_authorizes_only_its_own_parcels() {
    let Some(f) = fixture().await else { return };
    let (world, owner, grantee) = (world_name(), Wallet::random(), Wallet::random());
    f.names.set(&owner.address, world.label());

    let seed = deployment(&owner, &world, "Seed", "9,9", &["9,9"]).await;
    post_entity(&f.app, &seed, 200).await;

    grant_parcels(&f.pool, &world, &grantee.address, &["0,0"]).await;

    let outside = deployment(&grantee, &world, "Outside", "0,0", &["0,0", "0,1"]).await;
    post_entity(&f.app, &outside, 403).await;
    assert!(
        !deployed_entity_ids(&f.pool, &world)
            .await
            .contains(&outside.entity_id),
        "a parcel-scoped grant must not authorize parcels outside it"
    );

    let inside = deployment(&grantee, &world, "Inside", "0,0", &["0,0"]).await;
    post_entity(&f.app, &inside, 200).await;
    assert!(
        deployed_entity_ids(&f.pool, &world)
            .await
            .contains(&inside.entity_id)
    );
}

#[tokio::test]
async fn replacing_a_scene_requires_authority_over_its_whole_footprint() {
    let Some(f) = fixture().await else { return };
    let (world, owner, grantee) = (world_name(), Wallet::random(), Wallet::random());
    f.names.set(&owner.address, world.label());

    let existing = deployment(&owner, &world, "Existing", "0,0", &["0,0", "0,1"]).await;
    post_entity(&f.app, &existing, 200).await;
    let untouched = deployment(&owner, &world, "Untouched", "7,7", &["7,7"]).await;
    post_entity(&f.app, &untouched, 200).await;

    grant_parcels(&f.pool, &world, &grantee.address, &["0,0"]).await;

    let partial = deployment(&grantee, &world, "Partial", "0,0", &["0,0"]).await;
    post_entity(&f.app, &partial, 403).await;
    let mut ids = deployed_entity_ids(&f.pool, &world).await;
    ids.sort();
    let mut expected = vec![existing.entity_id.clone(), untouched.entity_id.clone()];
    expected.sort();
    assert_eq!(ids, expected, "the 403 must leave both scenes in place");

    grant_parcels(&f.pool, &world, &grantee.address, &["0,0", "0,1"]).await;
    let full = deployment(&grantee, &world, "Full", "0,0", &["0,0"]).await;
    post_entity(&f.app, &full, 200).await;

    let mut ids = deployed_entity_ids(&f.pool, &world).await;
    ids.sort();
    let mut expected = vec![full.entity_id.clone(), untouched.entity_id.clone()];
    expected.sort();
    assert_eq!(
        ids, expected,
        "only the authorized scene identity may be replaced"
    );
}

#[tokio::test]
async fn a_scene_may_not_declare_a_base_parcel_outside_itself() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random());
    f.names.set(&owner.address, world.label());

    let liar = deployment(&owner, &world, "Liar", "50,50", &["0,0", "0,1"]).await;
    let body = post_entity(&f.app, &liar, 400).await;
    let errors = body["errors"].as_array().expect("errors array");
    assert!(
        errors.iter().any(|e| e.as_str()
            == Some(
                "The scene base parcel [50,50] must be included in the scene parcels [0,0, 0,1]."
            )),
        "expected the base-not-in-parcels error, got {errors:?}"
    );
    assert!(deployed_entity_ids(&f.pool, &world).await.is_empty());

    let honest = deployment(&owner, &world, "Honest", "0,1", &["0,0", "0,1"]).await;
    post_entity(&f.app, &honest, 200).await;
}

#[tokio::test]
async fn unpublishing_requires_authority_over_the_whole_scene_footprint() {
    let Some(f) = fixture().await else { return };
    let (world, owner, grantee) = (world_name(), Wallet::random(), Wallet::random());
    f.names.set(&owner.address, world.label());

    let wide = deployment(&owner, &world, "Wide", "0,0", &["0,0", "0,1"]).await;
    post_entity(&f.app, &wide, 200).await;

    grant_parcels(&f.pool, &world, &grantee.address, &["0,0"]).await;

    let path = format!("/world/{}/scenes/0,0", world.display());
    expect(
        &f.app,
        Case::new("delete", &path).signed(&grantee).await,
        403,
    )
    .await;
    assert_eq!(
        deployed_entity_ids(&f.pool, &world).await,
        vec![wide.entity_id.clone()],
        "a grant on one parcel must not unpublish a scene that spans two"
    );

    grant_parcels(&f.pool, &world, &grantee.address, &["0,0", "0,1"]).await;
    expect(
        &f.app,
        Case::new("delete", &path).signed(&grantee).await,
        200,
    )
    .await;
    assert!(deployed_entity_ids(&f.pool, &world).await.is_empty());
}

fn bare_scene(base: &str, parcels: &[&str]) -> Value {
    json!({ "type": "scene", "timestamp": 1000, "pointers": parcels, "metadata": { "scene": { "base": base, "parcels": parcels } } })
}

#[tokio::test]
async fn a_base_outside_the_scene_footprint_never_becomes_the_scene_identity() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafyliar",
        &bare_scene("100,100", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .expect("deploy liar");
    assert_eq!(
        store::scene_base_parcel(&f.pool, &world, "bafyliar")
            .await
            .unwrap(),
        Some("0,0".to_string()),
        "a base outside the footprint must fall back to the scene's own first parcel"
    );
    deploy(
        &f,
        &world,
        &owner,
        "bafyhonest",
        &bare_scene("1,1", &["1,0", "1,1"]),
        &["1,0", "1,1"],
        &Deploy::Owner,
    )
    .await
    .expect("deploy honest");
    assert_eq!(
        store::scene_base_parcel(&f.pool, &world, "bafyhonest")
            .await
            .unwrap(),
        Some("1,1".to_string())
    );
    let bases: Vec<(String, Option<String>)> = store::list_scenes(&f.pool, &world)
        .await
        .unwrap()
        .into_iter()
        .map(|(id, _, base)| (id, base))
        .collect();
    assert!(bases.contains(&("bafyliar".to_string(), Some("0,0".to_string()))));
    assert!(bases.contains(&("bafyhonest".to_string(), Some("1,1".to_string()))));
    // The resolver agrees: the liar's base is its first parcel; each scene
    // has its own storage scope whatever its base claims.
    let active = rooms::world_scene(&f.pool, &world, &SceneId::parse("bafyliar").unwrap())
        .await
        .unwrap()
        .unwrap();
    let honest = rooms::world_scene(&f.pool, &world, &SceneId::parse("bafyhonest").unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.base.canonical(), "0,0");
    assert!(active.namespace.is_some() && honest.namespace.is_some());
    assert_ne!(active.namespace, honest.namespace);
    assert!(active.local);
}

#[tokio::test]
async fn a_scoped_deploy_replaces_only_the_authorized_identities() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafyexisting",
        &bare_scene("0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    deploy(
        &f,
        &world,
        &owner,
        "bafyneighbour",
        &bare_scene("5,5", &["5,5"]),
        &["5,5"],
        &Deploy::Owner,
    )
    .await
    .unwrap();

    let overlapping = f
        .state
        .policy
        .overlapping_scenes(&world, &parcels(&["0,0"]))
        .await
        .unwrap();
    assert_eq!(
        overlapping,
        vec![(
            SceneId::parse("bafyexisting").unwrap(),
            parcels(&["0,0", "0,1"])
        )],
        "the overlap query must report the replaced scene's WHOLE footprint"
    );

    deploy(
        &f,
        &world,
        &owner,
        "bafynew",
        &bare_scene("0,0", &["0,0"]),
        &["0,0"],
        &Deploy::Scoped(vec![SceneId::parse("bafyexisting").unwrap()]),
    )
    .await
    .expect("scoped deploy");
    assert_eq!(
        entity_ids(&f, &world).await,
        vec!["bafyneighbour".to_string(), "bafynew".to_string()],
        "only the authorized identity may be replaced"
    );
}

#[tokio::test]
async fn a_scoped_deploy_rolls_back_when_an_unauthorized_scene_survives() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafyexisting",
        &bare_scene("0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let err = deploy(
        &f,
        &world,
        &owner,
        "bafyracer",
        &bare_scene("0,0", &["0,0"]),
        &["0,0"],
        &Deploy::Scoped(Vec::new()),
    )
    .await
    .expect_err("stale authorization must not deploy");
    assert!(
        err.is_conflict() && err.to_string().contains(world.key()),
        "expected a replacement conflict naming {}, got {err:?}",
        world.key()
    );
    assert_eq!(
        entity_ids(&f, &world).await,
        vec!["bafyexisting".to_string()],
        "the whole transaction must roll back"
    );
}

fn scene_at(timestamp: i64, parcels: &[&str]) -> Value {
    json!({ "type": "scene", "timestamp": timestamp, "pointers": parcels, "metadata": { "scene": { "base": parcels[0], "parcels": parcels } } })
}

fn assert_newer_refusal(result: Result<(), super::http::ApiError>) {
    let err = result.expect_err("an older entity must not replace a newer scene");
    assert_eq!(err.status(), 400, "{err:?}");
    assert_eq!(
        err.public_message(),
        "Deployment failed: a newer scene is already deployed on one or more of these parcels."
    );
}

#[tokio::test]
async fn an_older_entity_never_replaces_a_newer_overlapping_scene() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafynewer",
        &scene_at(2000, &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();

    assert_newer_refusal(
        deploy(
            &f,
            &world,
            &owner,
            "bafyolder",
            &scene_at(1000, &["0,0", "0,1"]),
            &["0,0", "0,1"],
            &Deploy::Owner,
        )
        .await,
    );
    assert_newer_refusal(
        deploy(
            &f,
            &world,
            &owner,
            "bafyolder",
            &scene_at(1000, &["0,0"]),
            &["0,0"],
            &Deploy::Scoped(vec![SceneId::parse("bafynewer").unwrap()]),
        )
        .await,
    );
    assert_eq!(entity_ids(&f, &world).await, vec!["bafynewer".to_string()]);

    deploy(
        &f,
        &world,
        &owner,
        "bafyelsewhere",
        &scene_at(1000, &["5,5"]),
        &["5,5"],
        &Deploy::Owner,
    )
    .await
    .expect("a non-overlapping older entity deploys");
    deploy(
        &f,
        &world,
        &owner,
        "bafynewer",
        &scene_at(2000, &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .expect("re-publishing the deployed entity is not older than itself");
    deploy(
        &f,
        &world,
        &owner,
        "bafylatest",
        &scene_at(3000, &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .expect("a newer entity replaces the scene");
    assert_eq!(
        entity_ids(&f, &world).await,
        vec!["bafyelsewhere".to_string(), "bafylatest".to_string()]
    );
}

#[tokio::test]
async fn equal_timestamps_break_ties_by_entity_id() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafym",
        &scene_at(1000, &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();

    assert_newer_refusal(
        deploy(
            &f,
            &world,
            &owner,
            "bafya",
            &scene_at(1000, &["0,0"]),
            &["0,0"],
            &Deploy::Owner,
        )
        .await,
    );
    assert_eq!(entity_ids(&f, &world).await, vec!["bafym".to_string()]);

    deploy(
        &f,
        &world,
        &owner,
        "bafyz",
        &scene_at(1000, &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .expect("a higher entity id wins the tie");
    assert_eq!(entity_ids(&f, &world).await, vec!["bafyz".to_string()]);
}

#[tokio::test]
async fn equal_timestamps_compare_entity_ids_byte_wise() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "QmLegacyScene",
        &scene_at(1000, &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();

    deploy(
        &f,
        &world,
        &owner,
        "bafylocal",
        &scene_at(1000, &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .expect("'b' sorts after 'Q' byte-wise");
    assert_eq!(entity_ids(&f, &world).await, vec!["bafylocal".to_string()]);
}

#[tokio::test]
async fn a_future_dated_scene_counts_as_dated_at_its_deploy() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    let ahead = chrono::Utc::now().timestamp_millis() + 10 * 60 * 1000;
    deploy(
        &f,
        &world,
        &owner,
        "bafyfuture",
        &scene_at(ahead, &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let now = chrono::Utc::now().timestamp_millis();

    assert_newer_refusal(
        deploy(
            &f,
            &world,
            &owner,
            "bafyolder",
            &scene_at(now - 60_000, &["0,0"]),
            &["0,0"],
            &Deploy::Owner,
        )
        .await,
    );
    deploy(
        &f,
        &world,
        &owner,
        "bafyowner",
        &scene_at(now, &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .expect("a publish signed after the future-dated deploy replaces it");
    assert_eq!(entity_ids(&f, &world).await, vec!["bafyowner".to_string()]);
}

#[tokio::test]
async fn a_stored_scene_without_a_numeric_timestamp_never_blocks() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    for (id, base) in [("bafytext", "0,0"), ("bafymissing", "1,0")] {
        deploy(
            &f,
            &world,
            &owner,
            id,
            &scene_at(5000, &[base]),
            &[base],
            &Deploy::Owner,
        )
        .await
        .unwrap();
    }
    sqlx::query("UPDATE worlds.world_scenes SET entity = jsonb_set(entity, '{timestamp}', '\"5000\"') WHERE entity_id = 'bafytext'")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE worlds.world_scenes SET entity = entity - 'timestamp' WHERE entity_id = 'bafymissing'")
        .execute(&f.pool)
        .await
        .unwrap();

    deploy(
        &f,
        &world,
        &owner,
        "bafyolder",
        &scene_at(1000, &["0,0", "1,0"]),
        &["0,0", "1,0"],
        &Deploy::Owner,
    )
    .await
    .expect("malformed stored timestamps do not block");
    assert_eq!(entity_ids(&f, &world).await, vec!["bafyolder".to_string()]);
}

#[tokio::test]
async fn an_entity_without_an_integer_timestamp_is_refused() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    let mut undated = bare_scene("0,0", &["0,0"]);
    undated.as_object_mut().unwrap().remove("timestamp");
    let err = deploy(
        &f,
        &world,
        &owner,
        "bafyundated",
        &undated,
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap_err();
    assert!(err.is_bad_request(), "{err:?}");
    assert!(entity_ids(&f, &world).await.is_empty());
}

#[tokio::test]
async fn a_newer_mirrored_scene_keeps_the_world_mirrored() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    sqlx::query(
        "INSERT INTO worlds.worlds (name, mirror_source) VALUES ($1, 'https://upstream.example')",
    )
    .bind(world.key())
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source) \
         VALUES ($1, 'bafymirrored', '[]'::json, $2, $3, ARRAY['0,0'], 1, 'https://upstream.example')",
    )
    .bind(world.key())
    .bind(scene_at(2000, &["0,0"]))
    .bind(crate::ZERO_ADDRESS)
    .execute(&f.pool)
    .await
    .unwrap();

    assert_newer_refusal(
        deploy(
            &f,
            &world,
            &owner,
            "bafylocal",
            &scene_at(1000, &["0,0"]),
            &["0,0"],
            &Deploy::Owner,
        )
        .await,
    );
    let mirror: Option<String> =
        sqlx::query_scalar("SELECT mirror_source FROM worlds.worlds WHERE name = $1")
            .bind(world.key())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(mirror.as_deref(), Some("https://upstream.example"));
    assert_eq!(
        deployed_entity_ids(&f.pool, &world).await,
        vec!["bafymirrored".to_string()]
    );
}

#[tokio::test]
async fn publishing_an_older_entity_answers_400_and_keeps_the_newer_scene() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random());
    f.names.set(&owner.address, world.label());
    let now = chrono::Utc::now().timestamp_millis();

    let newer = deployment_at(
        &owner,
        &world,
        "Newer",
        "0,0",
        &["0,0"],
        "thumb.png",
        b"n",
        now,
    )
    .await;
    post_entity(&f.app, &newer, 200).await;
    let older = deployment_at(
        &owner,
        &world,
        "Older",
        "0,0",
        &["0,0"],
        "thumb.png",
        b"o",
        now - 10_000,
    )
    .await;
    let body = post_entity(&f.app, &older, 400).await;
    assert_eq!(
        body,
        json!({ "errors": ["Deployment failed: a newer scene is already deployed on one or more of these parcels."] })
    );
    assert_eq!(
        deployed_entity_ids(&f.pool, &world).await,
        vec![newer.entity_id]
    );
}

async fn blocked_since(pool: &PgPool, world: &WorldName) -> Option<chrono::NaiveDateTime> {
    sqlx::query_scalar("SELECT blocked_since FROM worlds.worlds WHERE name = $1")
        .bind(world.key())
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn about_refuses(f: &Fixture, world: &WorldName) -> bool {
    let about = crate::queries::world_about(&f.pool, world.key())
        .await
        .unwrap();
    let cfg = crate::Settings::for_tests("https://worlds.example.org");
    matches!(
        crate::about::build_about(&cfg, world.key(), about),
        Err(crate::content::errors::AppError::Unauthorized(_))
    )
}

#[tokio::test]
async fn a_disabled_world_refuses_publishes_until_enabled() {
    let Some(f) = fixture().await else { return };
    let (world, owner, grantee) = (world_name(), Wallet::random(), Wallet::random());
    f.names.set(&owner.address, world.label());
    post_entity(
        &f.app,
        &deployment(&owner, &world, "Seed", "0,0", &["0,0"]).await,
        200,
    )
    .await;
    let seeded = deployed_entity_ids(&f.pool, &world).await;

    expect(
        &f.app,
        Case::new("post", format!("/admin/worlds/{}/disable", world.key())).bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    let disabled_at = blocked_since(&f.pool, &world).await;
    assert!(disabled_at.is_some());

    let refused = post_entity(
        &f.app,
        &deployment(&owner, &world, "Owner", "0,0", &["0,0"]).await,
        403,
    )
    .await;
    assert!(
        refused["errors"][0]
            .as_str()
            .unwrap()
            .contains("is disabled and does not accept deployments"),
        "{refused}"
    );
    grant_parcels(&f.pool, &world, &grantee.address, &["3,3"]).await;
    post_entity(
        &f.app,
        &deployment(&grantee, &world, "Scoped", "3,3", &["3,3"]).await,
        403,
    )
    .await;
    assert_eq!(
        deployed_entity_ids(&f.pool, &world).await,
        seeded,
        "nothing was published"
    );
    assert_eq!(blocked_since(&f.pool, &world).await, disabled_at);
    assert!(about_refuses(&f, &world).await);

    expect(
        &f.app,
        Case::new("post", format!("/admin/worlds/{}/enable", world.key())).bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    post_entity(
        &f.app,
        &deployment(&owner, &world, "Owner", "0,0", &["0,0"]).await,
        200,
    )
    .await;
}

#[tokio::test]
async fn a_disable_racing_a_publish_is_refused_inside_the_transaction() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafyseed",
        &bare_scene("0,0", &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    store::admin_set_world_blocked(&f.pool, &world, true)
        .await
        .unwrap();
    let err = deploy(
        &f,
        &world,
        &owner,
        "bafynext",
        &scene_at(2000, &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap_err();
    assert_eq!(err.status(), 403);
    assert_eq!(deployed_entity_ids(&f.pool, &world).await, ["bafyseed"]);
}

#[tokio::test]
async fn undeploy_scene_removes_only_the_authorized_identities() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafywide",
        &bare_scene("0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    deploy(
        &f,
        &world,
        &owner,
        "bafyother",
        &bare_scene("5,5", &["5,5"]),
        &["5,5"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let at = Parcel::parse("0,0").unwrap();
    assert_eq!(
        store::undeploy_scene(&f.pool, &world, at, Some(&[]))
            .await
            .unwrap(),
        0,
        "an empty authorization set deletes nothing"
    );
    assert_eq!(entity_ids(&f, &world).await.len(), 2);
    assert_eq!(
        store::undeploy_scene(
            &f.pool,
            &world,
            at,
            Some(&[SceneId::parse("bafywide").unwrap()])
        )
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        entity_ids(&f, &world).await,
        vec!["bafyother".to_string()],
        "the unauthorized neighbour must survive"
    );
    assert_eq!(
        store::undeploy_scene(&f.pool, &world, Parcel::parse("5,5").unwrap(), None)
            .await
            .unwrap(),
        1
    );
    assert!(entity_ids(&f, &world).await.is_empty());
}

/// `(identity, parcels, active)` of a world's storage identities, oldest first.
async fn scene_identities(f: &Fixture, world: &WorldName) -> Vec<(Uuid, Vec<String>, bool)> {
    sqlx::query_as(
        "SELECT identity, parcels, active FROM explore.world_scene_place WHERE world_name = $1 ORDER BY created_at, identity",
    )
    .bind(world.key())
    .fetch_all(&f.pool)
    .await
    .unwrap()
}

async fn scene_identity(f: &Fixture, world: &WorldName, id: &str) -> Option<Uuid> {
    rooms::world_scene(&f.pool, world, &SceneId::parse(id).unwrap())
        .await
        .unwrap()
        .and_then(|d| d.namespace)
        .map(|ns| Uuid::parse_str(&ns.scene_namespace()).unwrap())
}

/// Two concurrent publishes splitting one footprint cannot share its identity.
#[tokio::test]
async fn concurrent_publishes_serialize_scene_identity_assignment() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafy-original",
        &bare_scene("0,0", &["0,0", "1,0"]),
        &["0,0", "1,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let original = scene_identity(&f, &world, "bafy-original").await.unwrap();
    let mut left = bare_scene("0,0", &["0,0"]);
    let mut right = bare_scene("1,0", &["1,0"]);
    left["timestamp"] = json!(2000);
    right["timestamp"] = json!(2000);
    let (a, b) = tokio::join!(
        deploy(
            &f,
            &world,
            &owner,
            "bafy-left",
            &left,
            &["0,0"],
            &Deploy::Owner
        ),
        deploy(
            &f,
            &world,
            &owner,
            "bafy-right",
            &right,
            &["1,0"],
            &Deploy::Owner
        ),
    );
    a.unwrap();
    b.unwrap();
    let a = scene_identity(&f, &world, "bafy-left").await.unwrap();
    let b = scene_identity(&f, &world, "bafy-right").await.unwrap();
    assert_ne!(a, b);
    assert!(a == original || b == original);
    let identities = scene_identities(&f, &world).await;
    assert_eq!(identities.len(), 2);
    assert!(identities.iter().all(|(_, _, active)| *active));
}

/// The publish and undeploy paths keep Places' overlap rule
/// (`scene_identity`): one overlapped identity is kept (a moved base too),
/// none or several mint a new one and retire the overlapped, an undeploy
/// retires its scene's; no storage row is ever deleted.
#[tokio::test]
async fn publishes_and_undeploys_keep_a_storage_identity_per_scene() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    let timed = |ts: i64, base: &str, parcels: &[&str]| {
        let mut scene = bare_scene(base, parcels);
        scene["timestamp"] = json!(ts);
        scene
    };
    let publish = |id: &'static str, scene: Value, footprint: &'static [&'static str]| {
        let (f, world, owner) = (&f, &world, &owner);
        async move {
            deploy(f, world, owner, id, &scene, footprint, &Deploy::Owner)
                .await
                .unwrap()
        }
    };
    publish("bafya", timed(1, "0,0", &["0,0", "0,1"]), &["0,0", "0,1"]).await;
    let a = scene_identity(&f, &world, "bafya").await.unwrap();
    sqlx::query(
        "INSERT INTO worlds.world_storage (realm, scene_namespace, key, value, value_size) VALUES ($1, $2, 'k', '1', 1)",
    )
    .bind(world.key())
    .bind(a.to_string())
    .execute(&f.pool)
    .await
    .unwrap();

    // A redeploy overlapping that scene alone keeps it, its base moved.
    publish("bafya2", timed(2, "0,2", &["0,1", "0,2"]), &["0,1", "0,2"]).await;
    assert_eq!(scene_identity(&f, &world, "bafya2").await, Some(a));
    assert_eq!(
        scene_identities(&f, &world).await,
        vec![(a, vec!["0,1".to_string(), "0,2".to_string()], true)]
    );

    // A scene overlapping none gets its own.
    publish("bafyb", timed(3, "5,5", &["5,5"]), &["5,5"]).await;
    let b = scene_identity(&f, &world, "bafyb").await.unwrap();
    assert_ne!(a, b);

    // One overlapping both gets a new one and retires both.
    publish("bafyc", timed(4, "5,5", &["0,2", "5,5"]), &["0,2", "5,5"]).await;
    let c = scene_identity(&f, &world, "bafyc").await.unwrap();
    assert!(c != a && c != b);
    let states: Vec<(Uuid, bool)> = scene_identities(&f, &world)
        .await
        .into_iter()
        .map(|(id, _, active)| (id, active))
        .collect();
    assert_eq!(states, vec![(a, false), (b, false), (c, true)]);

    // A publish beside it, then an undeploy of each: every identity retires.
    publish("bafyd", timed(5, "9,9", &["9,9"]), &["9,9"]).await;
    let d = scene_identity(&f, &world, "bafyd").await.unwrap();
    store::undeploy_scene(&f.pool, &world, Parcel::parse("0,2").unwrap(), None)
        .await
        .unwrap();
    let active: Vec<Uuid> = scene_identities(&f, &world)
        .await
        .into_iter()
        .filter_map(|(id, _, active)| active.then_some(id))
        .collect();
    assert_eq!(
        active,
        vec![d],
        "the undeploy retired its scene's identity only"
    );
    store::undeploy_world(&f.pool, &world).await.unwrap();
    assert!(
        scene_identities(&f, &world)
            .await
            .iter()
            .all(|(_, _, active)| !active)
    );
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM worlds.world_storage WHERE realm = $1 AND scene_namespace = $2",
    )
    .bind(world.key())
    .bind(a.to_string())
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(rows, 1, "storage rows outlive their identity");
}

async fn store_png(f: &Fixture) -> String {
    let hash = hash_bytes_v1(&PNG_MAGIC);
    f.storage
        .store(&hash, bytes::Bytes::from_static(&PNG_MAGIC))
        .await
        .unwrap();
    hash
}

#[tokio::test]
async fn deploy_populates_settings_and_read_surfaces() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    let thumb = store_png(&f).await;
    deploy(
        &f,
        &world,
        &owner,
        "bafyentity",
        &scene_entity(&world, "My World", &thumb, "0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .expect("deploy_scene");

    let settings = store::get_world_settings(&f.pool, &world)
        .await
        .unwrap()
        .expect("settings");
    assert_eq!(settings.title.as_deref(), Some("My World"));
    assert_eq!(settings.content_rating.as_deref(), Some("E"));
    assert_eq!(settings.skybox_time, Some(36000));
    assert_eq!(
        settings.categories.as_deref(),
        Some(&["art".to_string(), "game".to_string()][..])
    );
    assert!(settings.single_player);
    assert!(
        settings.show_in_places,
        "no placesConfig.optOut declared: the column's default"
    );
    assert_eq!(settings.thumbnail_hash.as_deref(), Some(thumb.as_str()));
    assert_eq!(settings.spawn_coordinates.as_deref(), Some("0,0"));
    assert_eq!(settings.settings_version, 0);
    assert_eq!(settings.access_type.as_deref(), Some("unrestricted"));

    let opts = |authorized_deployer: Option<String>, has_deployed_scenes: Option<bool>| {
        (
            WorldsListFilters {
                authorized_deployer,
                search: Some(world.key().to_string()),
                has_deployed_scenes,
            },
            WorldsListOptions {
                limit: 50,
                offset: 0,
                order_by: WorldsOrderBy::Name,
                order_direction: OrderDirection::Asc,
            },
        )
    };
    let (filters, options) = opts(None, None);
    let (worlds, total) = store::list_worlds_public(&f.pool, &filters, &options)
        .await
        .unwrap();
    assert_eq!(total, 1);
    let w = &worlds[0];
    assert_eq!(w.name, world.key());
    assert_eq!(w.deployed_scenes, 1);
    assert!(w.last_deployed_at.is_some());
    assert!(w.single_player);
    assert!(w.show_in_places);
    assert_eq!(
        (w.min_x, w.max_x, w.min_y, w.max_y),
        (Some(0), Some(0), Some(0), Some(1))
    );

    let (filters, options) = opts(None, Some(true));
    assert_eq!(
        store::list_worlds_public(&f.pool, &filters, &options)
            .await
            .unwrap()
            .0
            .len(),
        1
    );
    let (filters, options) = opts(None, Some(false));
    assert!(
        store::list_worlds_public(&f.pool, &filters, &options)
            .await
            .unwrap()
            .0
            .is_empty()
    );
    let (filters, options) = opts(Some(owner.as_str().to_string()), None);
    assert_eq!(
        store::list_worlds_public(&f.pool, &filters, &options)
            .await
            .unwrap()
            .0
            .len(),
        1
    );
    let (filters, options) = opts(
        Some("0x2222222222222222222222222222222222222222".into()),
        None,
    );
    assert!(
        store::list_worlds_public(&f.pool, &filters, &options)
            .await
            .unwrap()
            .0
            .is_empty()
    );

    let manifest = store::world_manifest(&f.pool, &world)
        .await
        .unwrap()
        .expect("manifest");
    assert_eq!(manifest.total, 2);
    assert_eq!(manifest.parcels, vec!["0,0".to_string(), "0,1".to_string()]);
    assert_eq!(manifest.spawn_coordinates.as_deref(), Some("0,0"));

    let body = expect(
        &f.app,
        Case::new("get", format!("/worlds?search={}", world.key())),
        200,
    )
    .await;
    assert_eq!(body["total"], 1);
    assert_eq!(
        body["worlds"][0]["shape"],
        json!({ "x1": 0, "x2": 0, "y1": 0, "y2": 1 })
    );
}

#[tokio::test]
async fn redeploy_refreshes_settings_only_when_scene_ends_up_sole_occupant() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafyfirst",
        &scene_entity(&world, "First Title", "bafymissing", "0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    deploy(
        &f,
        &world,
        &owner,
        "bafyaside",
        &scene_entity(&world, "Aside Title", "bafymissing", "5,5", &["5,5"]),
        &["5,5"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let settings = store::get_world_settings(&f.pool, &world)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        settings.title.as_deref(),
        Some("First Title"),
        "a deploy leaving another scene in place must not refresh settings"
    );
    assert_eq!(settings.settings_version, 0);

    deploy(
        &f,
        &world,
        &owner,
        "bafytakeover",
        &scene_entity(
            &world,
            "Takeover Title",
            "bafymissing",
            "0,0",
            &["0,0", "0,1", "5,5"],
        ),
        &["0,0", "0,1", "5,5"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let settings = store::get_world_settings(&f.pool, &world)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        settings.title.as_deref(),
        Some("Takeover Title"),
        "a deploy replacing every deployed scene must refresh settings"
    );
    assert_eq!(settings.settings_version, 1);
}

#[tokio::test]
async fn redeploy_preserves_owner_settings_the_scene_does_not_express() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafyseed",
        &scene_entity(&world, "Seed Title", "bafymissing", "0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let bare = json!({ "type": "scene", "timestamp": 2000, "pointers": ["0,0", "0,1"], "metadata": { "scene": { "base": "0,0", "parcels": ["0,0", "0,1"] } } });
    deploy(
        &f,
        &world,
        &owner,
        "bafybare",
        &bare,
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let settings = store::get_world_settings(&f.pool, &world)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        settings.title.as_deref(),
        Some("Seed Title"),
        "a field the redeploying scene does not declare must keep its stored value"
    );
    assert_eq!(settings.content_rating.as_deref(), Some("E"));
    assert!(settings.single_player);
    assert_eq!(
        settings.settings_version, 0,
        "a refresh that changes nothing must not bump the settings version"
    );
}

#[tokio::test]
async fn deploy_rejects_out_of_policy_scene_metadata() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    let svg = bytes::Bytes::from_static(b"<svg xmlns=\"a\"/>");
    let not_image = hash_bytes_v1(&svg);
    f.storage.store(&not_image, svg).await.unwrap();
    let entity = json!({
        "type": "scene",
        "timestamp": 1000,
        "pointers": ["0,0"],
        "content": [{ "file": "thumb.png", "hash": not_image }],
        "metadata": {
            "display": { "title": "ab", "description": "xy", "navmapThumbnail": "thumb.png" },
            "worldConfiguration": { "name": world.display(), "skyboxConfig": { "fixedTime": 99999999999i64 } },
            "scene": { "base": "0,0", "parcels": ["0,0"] },
            "tags": ["ok", 7],
            "rating": "XXX"
        }
    });
    deploy(
        &f,
        &world,
        &owner,
        "bafypolicy",
        &entity,
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let settings = store::get_world_settings(&f.pool, &world)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(settings.title, None, "a 2-char title is out of policy");
    assert_eq!(settings.description, None);
    assert_eq!(settings.content_rating, None);
    assert_eq!(
        settings.skybox_time, None,
        "an over-i32 fixedTime must not wrap"
    );
    assert_eq!(
        settings.categories, None,
        "a non-string tag rejects the whole array"
    );
    assert!(
        !settings.single_player,
        "no fixedAdapter declared: the column's default"
    );
    assert!(settings.show_in_places);
    assert_eq!(
        settings.thumbnail_hash, None,
        "a non-image navmapThumbnail must not be promoted into settings"
    );
}

#[tokio::test]
async fn settings_version_tracks_settings_writes() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    let version = || async {
        store::get_world_settings(&f.pool, &world)
            .await
            .unwrap()
            .unwrap()
            .settings_version
    };
    let fp = ["0,0", "0,1"];
    deploy(
        &f,
        &world,
        &owner,
        "bafyver1",
        &scene_entity(&world, "Version World", "bafymissing", "0,0", &fp),
        &fp,
        &Deploy::Owner,
    )
    .await
    .unwrap();
    assert_eq!(version().await, 0, "first deploy starts at the default");
    deploy(
        &f,
        &world,
        &owner,
        "bafyver2",
        &scene_entity(&world, "Version World", "bafymissing", "0,0", &fp),
        &fp,
        &Deploy::Owner,
    )
    .await
    .unwrap();
    assert_eq!(version().await, 0, "an unchanged republish must not bump");
    deploy(
        &f,
        &world,
        &owner,
        "bafyver3",
        &scene_entity(&world, "Version World Renamed", "bafymissing", "0,0", &fp),
        &fp,
        &Deploy::Owner,
    )
    .await
    .unwrap();
    assert_eq!(version().await, 1, "a settings-changing redeploy bumps");

    let (after_put, _) = store::update_world_settings(
        &f.pool,
        &world,
        &owner,
        &WorldSettingsUpdate {
            title: Some("Owner Title".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        after_put.settings_version, 2,
        "PUT with a settings field bumps"
    );
    let (after_spawn, _) = store::update_world_settings(
        &f.pool,
        &world,
        &owner,
        &WorldSettingsUpdate {
            spawn_coordinates: Some("0,1".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        after_spawn.settings_version, 2,
        "spawn coordinates are not a versioned settings column"
    );
    store::store_access_for_owner(&f.pool, &world, &owner, &AccessSetting::Unrestricted)
        .await
        .unwrap();
    assert_eq!(version().await, 3, "an access change bumps");
}

#[tokio::test]
async fn settings_spawn_validation_runs_inside_the_update_transaction() {
    let Some(f) = fixture().await else { return };
    let owner = Wallet::random().address;
    let spawn_update = |spawn: &str| WorldSettingsUpdate {
        spawn_coordinates: Some(spawn.into()),
        ..Default::default()
    };
    let bad_request = |err: super::http::ApiError| {
        assert_eq!(err.status(), 400, "{err:?}");
        err.public_message()
    };

    let spawnless = world_name();
    let err = store::update_world_settings(&f.pool, &spawnless, &owner, &spawn_update("0,0"))
        .await
        .expect_err("a world without scenes cannot take a spawn");
    assert!(bad_request(err).contains("has no deployed scenes"));
    assert!(
        store::get_world(&f.pool, &spawnless)
            .await
            .unwrap()
            .is_none(),
        "the row materialized for the lock rolls back with the failed validation"
    );

    let world = world_name();
    deploy(
        &f,
        &world,
        &owner,
        "bafyspawn",
        &scene_entity(&world, "Spawn World", "bafymissing", "0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let err = store::update_world_settings(&f.pool, &world, &owner, &spawn_update("5,5"))
        .await
        .expect_err("a spawn outside the world shape is refused");
    assert!(bad_request(err).contains("must be within the world shape rectangle"));
    assert_eq!(
        store::get_world_settings(&f.pool, &world)
            .await
            .unwrap()
            .unwrap()
            .spawn_coordinates
            .as_deref(),
        Some("0,0")
    );
    let (updated, old_spawn) =
        store::update_world_settings(&f.pool, &world, &owner, &spawn_update("0,1"))
            .await
            .expect("in-shape spawn");
    assert_eq!(updated.spawn_coordinates.as_deref(), Some("0,1"));
    assert_eq!(old_spawn.as_deref(), Some("0,0"));
}

#[tokio::test]
async fn permission_parcels_lifecycle() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let owner = Wallet::random().address;
    let deployer = Wallet::random().address;
    let streamer = Wallet::random().address;
    store::ensure_world(&f.pool, &world, &owner).await.unwrap();

    let added = store::grant_addresses_world_wide_permission(
        &f.pool,
        &world,
        Permission::Deployment,
        std::slice::from_ref(&deployer),
        None,
    )
    .await
    .unwrap();
    assert_eq!(added, vec![deployer.as_str().to_string()]);
    assert!(
        f.state
            .policy
            .has_world_wide_permission(&world, Permission::Deployment, &deployer)
            .await
            .unwrap()
    );

    let created = store::add_parcels_to_permission(
        &f.pool,
        &world,
        Permission::Streaming,
        &streamer,
        &parcels(&["00,00", "1,1"]),
    )
    .await
    .unwrap();
    assert!(created);

    let records = store::permission_records(&f.pool, &world).await.unwrap();
    let dep = records
        .iter()
        .find(|r| r.permission == "deployment")
        .unwrap();
    assert!(dep.is_world_wide);
    assert_eq!(dep.parcel_count, 0);
    let strm = records
        .iter()
        .find(|r| r.permission == "streaming")
        .unwrap();
    assert!(!strm.is_world_wide);
    assert_eq!(strm.parcel_count, 2);
    assert!(
        !f.state
            .policy
            .has_world_wide_permission(&world, Permission::Streaming, &streamer)
            .await
            .unwrap()
    );

    let (total, listed) = store::parcels_for_permission(
        &f.pool,
        &world,
        Permission::Streaming,
        &streamer,
        100,
        0,
        None,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(total, 2);
    assert_eq!(listed, vec!["0,0".to_string(), "1,1".to_string()]);
    let (_, boxed) = store::parcels_for_permission(
        &f.pool,
        &world,
        Permission::Streaming,
        &streamer,
        100,
        0,
        Some((0, 0, 0, 0)),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(boxed, vec!["0,0".to_string()]);

    let (atot, addrs) = store::addresses_for_parcel_permission(
        &f.pool,
        &world,
        Permission::Streaming,
        &parcels(&["0,0"]),
        100,
        0,
    )
    .await
    .unwrap();
    assert_eq!(atot, 1);
    assert_eq!(addrs, vec![streamer.as_str().to_string()]);

    assert!(
        store::remove_parcels_from_permission(
            &f.pool,
            &world,
            Permission::Streaming,
            &streamer,
            &parcels(&["0,0"])
        )
        .await
        .unwrap()
    );
    let records = store::permission_records(&f.pool, &world).await.unwrap();
    let strm = records
        .iter()
        .find(|r| r.permission == "streaming")
        .unwrap();
    assert!(!strm.is_world_wide);
    assert_eq!(strm.parcel_count, 1);

    // Removing the last parcel drops the grant instead of widening it.
    assert!(
        store::remove_parcels_from_permission(
            &f.pool,
            &world,
            Permission::Streaming,
            &streamer,
            &parcels(&["1,1"])
        )
        .await
        .unwrap()
    );
    let records = store::permission_records(&f.pool, &world).await.unwrap();
    assert!(records.iter().all(|r| r.permission != "streaming"));
    assert!(
        !f.state
            .policy
            .has_world_wide_permission(&world, Permission::Streaming, &streamer)
            .await
            .unwrap()
    );
    assert!(
        !store::remove_parcels_from_permission(
            &f.pool,
            &world,
            Permission::Streaming,
            &streamer,
            &parcels(&["1,1"])
        )
        .await
        .unwrap()
    );

    // A world-wide grant is untouched by removing parcels it never had.
    assert!(
        store::remove_parcels_from_permission(
            &f.pool,
            &world,
            Permission::Deployment,
            &deployer,
            &parcels(&["5,5"])
        )
        .await
        .unwrap()
    );
    assert!(
        f.state
            .policy
            .has_world_wide_permission(&world, Permission::Deployment, &deployer)
            .await
            .unwrap()
    );

    let removed = store::remove_addresses_permission(
        &f.pool,
        &world,
        Permission::Deployment,
        std::slice::from_ref(&deployer),
    )
    .await
    .unwrap();
    assert_eq!(removed, vec![deployer.as_str().to_string()]);
}

#[tokio::test]
async fn single_statement_permission_routes_keep_their_row_shapes() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let owner = Wallet::random().address;
    let (a, b, c) = (
        Wallet::random().address,
        Wallet::random().address,
        Wallet::random().address,
    );

    assert!(store::get_world(&f.pool, &world).await.unwrap().is_none());
    assert!(
        store::permission_records(&f.pool, &world)
            .await
            .unwrap()
            .is_empty()
    );

    store::replace_world_wide_permission(
        &f.pool,
        &world,
        &owner,
        Permission::Deployment,
        &[a.clone(), b.clone()],
    )
    .await
    .unwrap();
    let added = store::grant_addresses_world_wide_permission(
        &f.pool,
        &world,
        Permission::Streaming,
        std::slice::from_ref(&c),
        None,
    )
    .await
    .unwrap();
    assert_eq!(added, vec![c.as_str().to_string()]);
    let added = store::grant_addresses_world_wide_permission(
        &f.pool,
        &world,
        Permission::Streaming,
        std::slice::from_ref(&c),
        None,
    )
    .await
    .unwrap();
    assert!(added.is_empty(), "a repeat grant adds nothing");

    assert_eq!(
        store::get_world(&f.pool, &world)
            .await
            .unwrap()
            .unwrap()
            .owner
            .as_ref(),
        Some(&owner)
    );
    assert_eq!(
        store::permission_records(&f.pool, &world)
            .await
            .unwrap()
            .len(),
        3
    );

    store::replace_world_wide_permission(
        &f.pool,
        &world,
        &owner,
        Permission::Deployment,
        &[a.clone(), c.clone()],
    )
    .await
    .unwrap();
    let records = store::permission_records(&f.pool, &world).await.unwrap();
    let deployment: Vec<&str> = records
        .iter()
        .filter(|r| r.permission == "deployment")
        .map(|r| r.address.as_str())
        .collect();
    let mut expected = vec![a.as_str(), c.as_str()];
    expected.sort();
    assert_eq!(deployment, expected);

    assert!(
        store::add_parcels_to_permission(
            &f.pool,
            &world,
            Permission::Deployment,
            &b,
            &parcels(&["00,00", "2,3"])
        )
        .await
        .unwrap()
    );
    assert!(
        !store::add_parcels_to_permission(
            &f.pool,
            &world,
            Permission::Deployment,
            &b,
            &parcels(&["2,3", "5,5"])
        )
        .await
        .unwrap()
    );
    let (total, page) =
        store::parcels_for_permission(&f.pool, &world, Permission::Deployment, &b, 2, 0, None)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        (total, page),
        (3, vec!["0,0".to_string(), "2,3".to_string()])
    );
    let (total, page) =
        store::parcels_for_permission(&f.pool, &world, Permission::Deployment, &b, 10, 10, None)
            .await
            .unwrap()
            .unwrap();
    assert_eq!((total, page), (3, Vec::new()));
    let (total, page) = store::parcels_for_permission(
        &f.pool,
        &world,
        Permission::Deployment,
        &b,
        10,
        0,
        Some((5, 5, 2, 3)),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        (total, page),
        (2, vec!["2,3".to_string(), "5,5".to_string()])
    );
    assert!(
        store::parcels_for_permission(&f.pool, &world, Permission::Streaming, &b, 10, 0, None)
            .await
            .unwrap()
            .is_none()
    );

    assert!(
        f.state
            .policy
            .has_permission_covering(&world, Permission::Deployment, &a, &parcels(&["9,9"]))
            .await
            .unwrap()
    );
    assert!(
        f.state
            .policy
            .has_permission_covering(
                &world,
                Permission::Deployment,
                &b,
                &parcels(&["0,0", "5,5"])
            )
            .await
            .unwrap()
    );
    assert!(
        !f.state
            .policy
            .has_permission_covering(
                &world,
                Permission::Deployment,
                &b,
                &parcels(&["0,0", "9,9"])
            )
            .await
            .unwrap()
    );
    assert!(
        !f.state
            .policy
            .has_permission_covering(&world, Permission::Deployment, &owner, &parcels(&["0,0"]))
            .await
            .unwrap()
    );

    let (atot, addrs) = store::addresses_for_parcel_permission(
        &f.pool,
        &world,
        Permission::Deployment,
        &parcels(&["0,0"]),
        1,
        0,
    )
    .await
    .unwrap();
    assert_eq!(atot, 3);
    assert_eq!(addrs.len(), 1);
    let (atot, addrs) = store::addresses_for_parcel_permission(
        &f.pool,
        &world,
        Permission::Deployment,
        &parcels(&["0,0"]),
        5,
        9,
    )
    .await
    .unwrap();
    assert_eq!((atot, addrs), (3, Vec::new()));

    assert!(
        store::remove_parcels_from_permission(
            &f.pool,
            &world,
            Permission::Deployment,
            &b,
            &parcels(&["0,0"])
        )
        .await
        .unwrap()
    );
    assert!(
        !store::remove_parcels_from_permission(
            &f.pool,
            &world,
            Permission::Streaming,
            &b,
            &parcels(&["0,0"])
        )
        .await
        .unwrap()
    );
    assert!(
        !store::remove_parcels_from_permission(
            &f.pool,
            &world,
            Permission::Streaming,
            &b,
            &ParcelSet::new()
        )
        .await
        .unwrap()
    );
    let (total, _) =
        store::parcels_for_permission(&f.pool, &world, Permission::Deployment, &b, 10, 0, None)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(total, 2);

    assert!(
        store::world_manifest(&f.pool, &world)
            .await
            .unwrap()
            .is_none(),
        "a world without a deployed scene has no manifest"
    );
}

#[derive(Default)]
struct RecordedRevocations(parking_lot::Mutex<Vec<super::Revocation>>);

#[async_trait::async_trait]
impl super::RevocationSink for RecordedRevocations {
    async fn revoke(&self, revocation: &super::Revocation) {
        self.0.lock().push(revocation.clone());
    }
}

impl RecordedRevocations {
    fn take(&self) -> Vec<super::Revocation> {
        std::mem::take(&mut *self.0.lock())
    }
}

async fn seed_community(pool: &PgPool, members: &[&Address]) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO social.communities (id, name, owner_address) VALUES ($1, 'c', $2)")
        .bind(id)
        .bind(Wallet::random().address)
        .execute(pool)
        .await
        .unwrap();
    for m in members {
        sqlx::query("INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'member')")
            .bind(id)
            .bind(m)
            .execute(pool)
            .await
            .unwrap();
    }
    id
}

async fn leave_community(pool: &PgPool, community: Uuid, member: &Address) {
    sqlx::query(
        "DELETE FROM social.community_members WHERE community_id = $1 AND member_address = $2",
    )
    .bind(community)
    .bind(member)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn community_allow_lists_require_membership_and_revoke_on_exit() {
    use super::Revocation;
    use crate::policy::{Access, AccessGrant};
    let Some(f) = fixture().await else { return };
    let recorded = Arc::new(RecordedRevocations::default());
    f.state.add_revocation_sink(recorded.clone());
    let world = world_name();
    let owner = Wallet::random();
    f.names.set(&owner.address, world.label());
    let (member, listed, granted, stranger) = (
        Wallet::random().address,
        Wallet::random().address,
        Wallet::random().address,
        Wallet::random().address,
    );
    let joined = seed_community(&f.pool, &[&owner.address, &member, &listed, &granted]).await;
    let other = seed_community(&f.pool, &[&owner.address, &member]).await;
    let foreign = seed_community(&f.pool, &[&member]).await;
    let w = world.display();
    let access_path = format!("/world/{w}/permissions/access");
    let community_path = |c: Uuid| format!("/world/{w}/permissions/access/communities/{c}");
    let not_member = format!("You are not a member of the following communities: {foreign}.");

    // Adding a community the signer is not a member of is refused on both routes.
    let meta = |communities: Vec<String>| {
        json!({ "type": "allow-list", "wallets": [listed.as_str(), granted.as_str()], "communities": communities })
            .to_string()
    };
    let body = expect(
        &f.app,
        Case::new("post", &access_path)
            .signed_meta(&owner, &meta(vec![joined.to_string(), foreign.to_string()]))
            .await,
        400,
    )
    .await;
    assert_eq!(body["message"], not_member);
    expect(
        &f.app,
        Case::new("post", &access_path)
            .signed_meta(&owner, &meta(vec![joined.to_string()]))
            .await,
        204,
    )
    .await;
    let body = expect(
        &f.app,
        Case::new("put", community_path(foreign))
            .signed(&owner)
            .await,
        400,
    )
    .await;
    assert_eq!(body["message"], not_member);
    expect(
        &f.app,
        Case::new("put", community_path(other)).signed(&owner).await,
        204,
    )
    .await;
    recorded.take();

    // Admission: a member by community, the listed wallet once, by its grant.
    let access = async |who: &Address| {
        let now = chrono::Utc::now().naive_utc();
        f.state
            .policy
            .can_access(&world, who, None, None, None, now)
            .await
            .unwrap()
    };
    assert_eq!(
        access(&member).await,
        Access::Allowed(AccessGrant::Community)
    );
    assert_eq!(
        access(&listed).await,
        Access::Allowed(AccessGrant::AllowListed)
    );
    assert_eq!(access(&stranger).await, Access::NotAllowed);

    // Removing a wallet grant keeps a wallet a listed community still admits.
    sqlx::query("INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) VALUES ($1, 'bafyc', '[]', '{}', 'owner', ARRAY['0,0'], 0)")
        .bind(world.key())
        .execute(&f.pool)
        .await
        .unwrap();
    expect(
        &f.app,
        Case::new("delete", format!("/world/{w}/permissions/access/{granted}"))
            .signed(&owner)
            .await,
        204,
    )
    .await;
    assert_eq!(recorded.take(), vec![]);
    assert_eq!(
        access(&granted).await,
        Access::Allowed(AccessGrant::Community)
    );

    // Leaving re-decides: the owner, the listed wallet and a member of
    // another listed community stay; a community no world lists kicks nobody.
    for who in [&owner.address, &listed, &member] {
        leave_community(&f.pool, joined, who).await;
        f.state.community_member_removed(joined, who).await;
    }
    f.state.community_member_removed(foreign, &member).await;
    assert_eq!(recorded.take(), vec![]);
    leave_community(&f.pool, other, &member).await;
    f.state.community_member_removed(other, &member).await;
    assert_eq!(
        recorded.take(),
        vec![Revocation::Address {
            world: world.clone(),
            address: member.clone()
        }]
    );

    // Deleting a community, or taking it off the list, re-decides everyone.
    f.state.community_closed(joined).await;
    assert_eq!(
        recorded.take(),
        vec![Revocation::AccessSetting {
            world: world.clone()
        }]
    );
    expect(
        &f.app,
        Case::new("delete", community_path(other))
            .signed(&owner)
            .await,
        204,
    )
    .await;
    assert_eq!(
        recorded.take(),
        vec![Revocation::AccessSetting {
            world: world.clone()
        }]
    );
}

/// A community exit reaches comms' LiveKit rooms: the wallet nothing else
/// admits is removed from the world's live room, a wallet grant keeps one.
#[tokio::test]
async fn a_community_exit_evicts_from_the_livekit_rooms() {
    use crate::comms::gatekeeper::{CommsState, Settings as CommsSettings};
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let room = crate::comms::rooms::worlds_service_scene_room(
        &world,
        &SceneId::parse("bafylivekit").unwrap(),
    );
    let requests = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
    let capture = requests.clone();
    let listed = room.clone();
    let fixture = Router::new().route(
        "/twirp/livekit.RoomService/{method}",
        axum::routing::post(
            move |axum::extract::Path(method): axum::extract::Path<String>,
                  axum::Json(body): axum::Json<Value>| {
                let (capture, listed) = (capture.clone(), listed.clone());
                async move {
                    capture.lock().unwrap().push((method.clone(), body));
                    axum::Json(match method.as_str() {
                        "ListRooms" => json!({ "rooms": [{ "name": listed, "sid": "RM_a", "metadata": "{}" }] }),
                        _ => json!({}),
                    })
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut settings = CommsSettings::for_tests();
    settings.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let comms = Arc::new(CommsState::new(
        f.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
        None,
        None,
        f.state.policy.clone(),
        Arc::new(rooms::SceneResolver::new(f.pool.clone())),
        Arc::new(crate::comms::presence::Presence::new()),
        Arc::new(crate::comms::interfaces::test_support::StubDirectory::default()),
    ));
    f.state
        .add_revocation_sink(Arc::new(super::CommsRoomRevocations(comms)));
    sqlx::query("INSERT INTO worlds.worlds (name, access_type) VALUES ($1, 'allow-list')")
        .bind(world.key())
        .execute(&f.pool)
        .await
        .unwrap();
    let (member, granted) = (Wallet::random().address, Wallet::random().address);
    sqlx::query("INSERT INTO worlds.world_permissions (world_name, permission, address) VALUES ($1, 'access', $2)")
        .bind(world.key())
        .bind(&granted)
        .execute(&f.pool)
        .await
        .unwrap();
    let community = seed_community(&f.pool, &[&member, &granted]).await;
    sqlx::query(
        "INSERT INTO worlds.world_access_communities (world_name, community_id) VALUES ($1, $2)",
    )
    .bind(world.key())
    .bind(community)
    .execute(&f.pool)
    .await
    .unwrap();

    for who in [&granted, &member] {
        leave_community(&f.pool, community, who).await;
        f.state.community_member_removed(community, who).await;
    }
    let removed = |who: &Address| {
        requests.lock().unwrap().iter().any(|(method, body)| {
            method == "RemoveParticipant"
                && body["room"] == room
                && body["identity"] == who.as_str()
        })
    };
    let evicted = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !removed(&member) {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(evicted.is_ok(), "{:?}", requests.lock().unwrap());
    assert!(!removed(&granted), "the wallet grant still admits it");
    server.abort();
}

#[tokio::test]
async fn allow_list_edits_are_guarded_updates() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let owner = Wallet::random().address;
    let (abc, def) = (Wallet::random().address, Wallet::random().address);
    let (c1, c2) = (Uuid::new_v4(), Uuid::new_v4());
    let version = || async {
        store::get_world_settings(&f.pool, &world)
            .await
            .unwrap()
            .unwrap()
            .settings_version
    };

    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::AddWallet(&abc))
            .await
            .unwrap(),
        AllowListEditOutcome::NotAllowList
    );
    store::store_access_for_owner(&f.pool, &world, &owner, &AccessSetting::Unrestricted)
        .await
        .unwrap();
    assert_eq!(version().await, 1);
    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::AddWallet(&abc))
            .await
            .unwrap(),
        AllowListEditOutcome::NotAllowList
    );

    store::store_access_for_owner(
        &f.pool,
        &world,
        &owner,
        &AccessSetting::AllowList {
            wallets: vec![abc.as_str().to_uppercase()],
            communities: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(version().await, 2);
    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::AddWallet(&abc))
            .await
            .unwrap(),
        AllowListEditOutcome::Applied,
        "presence still counts as a write"
    );
    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::AddWallet(&def))
            .await
            .unwrap(),
        AllowListEditOutcome::Applied
    );
    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::AddCommunity(c1))
            .await
            .unwrap(),
        AllowListEditOutcome::Applied
    );
    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::AddCommunity(c2))
            .await
            .unwrap(),
        AllowListEditOutcome::Applied
    );
    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::RemoveWallet(&abc))
            .await
            .unwrap(),
        AllowListEditOutcome::Applied
    );
    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::RemoveCommunity(c1))
            .await
            .unwrap(),
        AllowListEditOutcome::Applied
    );
    assert_eq!(version().await, 8);
    match store::get_world(&f.pool, &world)
        .await
        .unwrap()
        .unwrap()
        .access
    {
        AccessSetting::AllowList {
            wallets,
            communities,
        } => {
            assert_eq!(wallets, vec![def.as_str().to_string()]);
            assert_eq!(communities, vec![c2.to_string()]);
        }
        other => panic!("unexpected access {other:?}"),
    }

    // Caps: the wallet list refuses its 1001st entry, communities their 51st.
    let many: Vec<String> = (0..999).map(|i| format!("0x{i:040x}")).collect();
    let communities: Vec<String> = (0..50).map(|_| Uuid::new_v4().to_string()).collect();
    store::store_access_for_owner(
        &f.pool,
        &world,
        &owner,
        &AccessSetting::AllowList {
            wallets: many,
            communities: communities.clone(),
        },
    )
    .await
    .unwrap();
    let last = Wallet::random().address;
    let too_many = Wallet::random().address;
    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::AddWallet(&last))
            .await
            .unwrap(),
        AllowListEditOutcome::Applied
    );
    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::AddWallet(&too_many))
            .await
            .unwrap(),
        AllowListEditOutcome::CapExceeded
    );
    assert_eq!(
        store::modify_allow_list_access(&f.pool, &world, AllowListEdit::AddWallet(&last))
            .await
            .unwrap(),
        AllowListEditOutcome::Applied
    );
    assert_eq!(
        store::modify_allow_list_access(
            &f.pool,
            &world,
            AllowListEdit::AddCommunity(Uuid::new_v4())
        )
        .await
        .unwrap(),
        AllowListEditOutcome::CapExceeded
    );
    assert_eq!(
        store::modify_allow_list_access(
            &f.pool,
            &world,
            AllowListEdit::AddCommunity(Uuid::parse_str(&communities[7]).unwrap())
        )
        .await
        .unwrap(),
        AllowListEditOutcome::Applied
    );
    // The policy reads the same rows: a listed wallet enters, a stranger does not.
    let now = chrono::Utc::now().naive_utc();
    assert_eq!(
        f.state
            .policy
            .can_access(&world, &last, None, None, None, now)
            .await
            .unwrap(),
        crate::policy::Access::Allowed(crate::policy::AccessGrant::AllowListed)
    );
    assert_eq!(
        f.state
            .policy
            .can_access(&world, &too_many, None, None, None, now)
            .await
            .unwrap(),
        crate::policy::Access::NotAllowed
    );
    // A type change drops the list: back to allow-list, nobody is listed.
    store::store_access_for_owner(&f.pool, &world, &owner, &AccessSetting::Unrestricted)
        .await
        .unwrap();
    store::store_access_for_owner(
        &f.pool,
        &world,
        &owner,
        &AccessSetting::AllowList {
            wallets: vec![],
            communities: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        f.state
            .policy
            .can_access(&world, &last, None, None, None, now)
            .await
            .unwrap(),
        crate::policy::Access::NotAllowed
    );
}

#[tokio::test]
async fn active_content_keys_names_every_referenced_blob_and_fails_closed_on_an_unreadable_entity()
{
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafyoldscene",
        &scene_entity(&world, "Old", "bafyoldthumb", "0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    // Same timestamp: the greater entity id wins the tie.
    deploy(
        &f,
        &world,
        &owner,
        "bafyupdatedscene",
        &scene_entity(&world, "New", "bafynewthumb", "0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE worlds.worlds SET thumbnail_hash = 'aabbccdd' WHERE name = $1")
        .bind(world.key())
        .execute(&f.pool)
        .await
        .unwrap();

    let keys = store::active_content_keys(&f.pool)
        .await
        .expect("active keys");
    for expected in ["bafyupdatedscene", "bafynewthumb", "aabbccdd"] {
        assert!(keys.contains(expected), "missing active key {expected}");
    }
    for replaced in ["bafyoldscene", "bafyoldthumb"] {
        assert!(
            !keys.contains(replaced),
            "replaced scene still active: {replaced}"
        );
    }

    // The set is a whole-table scan, so the fail-closed half runs in the same
    // test (a broken row must not be visible to the shared database's other
    // tests while they read the set).
    let (world, owner) = (world_name(), Wallet::random().address);
    let id = format!("bafybroken{}", Uuid::new_v4().simple());
    deploy(
        &f,
        &world,
        &owner,
        &id,
        &scene_entity(&world, "Broken", "bafythumb", "0,0", &["0,0"]),
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE worlds.world_scenes SET entity = $1::jsonb WHERE entity_id = $2")
        .bind(r#"{"content": [{"file": "a.png"}]}"#)
        .bind(&id)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        store::active_content_keys(&f.pool).await.is_err(),
        "a content entry without a hash must abort the whole active set"
    );
    sqlx::query("UPDATE worlds.world_scenes SET entity = 'null'::jsonb WHERE entity_id = $1")
        .bind(&id)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        store::active_content_keys(&f.pool).await.is_err(),
        "a non-object entity must abort the whole active set"
    );
    sqlx::query("DELETE FROM worlds.world_scenes WHERE entity_id = $1")
        .bind(&id)
        .execute(&f.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn scoped_replacement_conflicts_on_unauthorized_overlap() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    let thumb = store_png(&f).await;
    deploy(
        &f,
        &world,
        &owner,
        "bafyA",
        &scene_entity(&world, "A", &thumb, "0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let conflict = deploy(
        &f,
        &world,
        &owner,
        "bafyB",
        &scene_entity(&world, "B", &thumb, "0,0", &["0,0"]),
        &["0,0"],
        &Deploy::Scoped(vec![]),
    )
    .await;
    assert!(
        matches!(&conflict, Err(e) if e.is_conflict()),
        "unauthorized overlap must 409, got {conflict:?}"
    );
    assert_eq!(entity_ids(&f, &world).await, vec!["bafyA".to_string()]);
    deploy(
        &f,
        &world,
        &owner,
        "bafyB",
        &scene_entity(&world, "B", &thumb, "0,0", &["0,0"]),
        &["0,0"],
        &Deploy::Scoped(vec![SceneId::parse("bafyA").unwrap()]),
    )
    .await
    .expect("deploy B replacing A");
    let scenes = store::list_scenes(&f.pool, &world).await.unwrap();
    assert_eq!(scenes.len(), 1);
    assert_eq!(scenes[0].0, "bafyB");
    assert_eq!(scenes[0].1, vec!["0,0".to_string()]);
}

/// A replaced scene keeps its base parcel for scene comms seven days; the
/// scene that replaced it, a never-deployed id and an expired one do not.
#[tokio::test]
async fn a_replaced_scene_stays_resolvable_for_the_retention_window() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    let base_of = |id: &str, at: chrono::NaiveDateTime| {
        let (pool, world, id) = (f.pool.clone(), world.clone(), SceneId::parse(id).unwrap());
        async move {
            rooms::replaced_world_scene(&pool, &world, &id, at)
                .await
                .unwrap()
                .map(|r| r.base.canonical())
        }
    };
    for (i, (id, base, footprint, replacement)) in [
        ("bafyold", "0,1", &["0,0", "0,1"][..], Deploy::Owner),
        ("bafymid", "0,0", &["0,0"][..], Deploy::Owner),
        (
            "bafynew",
            "0,0",
            &["0,0"][..],
            Deploy::Scoped(vec![SceneId::parse("bafymid").unwrap()]),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut entity = scene_entity(&world, id, "bafymissing", base, footprint);
        entity["timestamp"] = json!(1000 + i);
        deploy(&f, &world, &owner, id, &entity, footprint, &replacement)
            .await
            .unwrap();
    }
    let now = chrono::Utc::now().naive_utc();
    assert_eq!(base_of("bafyold", now).await.as_deref(), Some("0,1"));
    assert_eq!(base_of("bafymid", now).await.as_deref(), Some("0,0"));
    assert_eq!(base_of("bafynew", now).await, None, "the active scene");
    assert_eq!(base_of("bafynever", now).await, None);
    let old =
        rooms::replaced_world_scene(&f.pool, &world, &SceneId::parse("bafyold").unwrap(), now)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        old.successors,
        [crate::comms::world_identity::Parcel::parse("0,0").unwrap()],
        "the active scene now on its parcels"
    );
    assert_eq!(
        base_of("bafyold", now + chrono::Duration::days(8)).await,
        None,
        "past the window"
    );

    sqlx::query(
        "UPDATE worlds.replaced_world_scenes SET replaced_at = replaced_at - interval '8 days' \
         WHERE world_name = $1 AND entity_id = 'bafyold'",
    )
    .bind(world.key())
    .execute(&f.pool)
    .await
    .unwrap();
    let other = world_name();
    sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
        .bind(other.key())
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO worlds.replaced_world_scenes (world_name, entity_id, parcels, replaced_at) \
         VALUES ($1, 'bafyelsewhere', '{0,0}', now() - interval '8 days')",
    )
    .bind(other.key())
    .execute(&f.pool)
    .await
    .unwrap();
    let mut last = scene_entity(&world, "last", "bafymissing", "0,0", &["0,0"]);
    last["timestamp"] = json!(1003);
    deploy(
        &f,
        &world,
        &owner,
        "bafylast",
        &last,
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let kept: Vec<String> = sqlx::query_scalar(
        "SELECT entity_id FROM worlds.replaced_world_scenes WHERE world_name = $1 ORDER BY entity_id",
    )
    .bind(world.key())
    .fetch_all(&f.pool)
    .await
    .unwrap();
    assert_eq!(kept, ["bafymid", "bafynew"], "a deploy purges expired rows");
    let elsewhere: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM worlds.replaced_world_scenes WHERE world_name = $1",
    )
    .bind(other.key())
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(elsewhere, 1, "the purge is scoped to the deploying world");
}

#[tokio::test]
async fn admin_pages_and_the_index_agree_on_the_scene_count() {
    let Some(f) = fixture().await else { return };
    let (world, owner) = (world_name(), Wallet::random().address);
    deploy(
        &f,
        &world,
        &owner,
        "bafyidx",
        &scene_entity(&world, "Indexed", "bafythumb", "0,0", &["0,0", "0,1"]),
        &["0,0", "0,1"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let (page, total) = store::admin_list_worlds_page(&f.pool, 10_000, 0)
        .await
        .unwrap();
    let row = page.iter().find(|w| w.name == world.key()).unwrap();
    assert_eq!(row.scene_count, 1);
    assert_eq!(total, page.len() as i64);
    let (empty, total_past_end) = store::admin_list_worlds_page(&f.pool, 10, 1_000_000)
        .await
        .unwrap();
    assert!(empty.is_empty());
    // The past-the-end page counts separately, and tests running beside this
    // one on the shared scratch database only add worlds in between.
    assert!(total_past_end >= total, "{total_past_end} < {total}");
}

#[tokio::test]
async fn manifest_keeps_total_order_limit_and_empty_worlds_in_one_query() {
    let Some(f) = fixture().await else { return };
    let mixed = world_name();
    let empty = world_name();
    sqlx::query(
        "INSERT INTO worlds.worlds (name, spawn_coordinates) VALUES ($1, '-2,0'), ($2, NULL)",
    )
    .bind(mixed.key())
    .bind(empty.key())
    .execute(&f.pool)
    .await
    .unwrap();
    let parcels: Vec<String> = (-3..510).rev().map(|i| format!("{i},0")).collect();
    for id in ["a", "b"] {
        sqlx::query("INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) VALUES ($1, $2, '[]', '{}', 'owner', $3, 0)")
            .bind(mixed.key())
            .bind(id)
            .bind(&parcels)
            .execute(&f.pool)
            .await
            .unwrap();
    }
    let upper = WorldName::parse(&mixed.display().to_uppercase()).unwrap();
    let manifest = store::world_manifest(&f.pool, &upper)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(manifest.total, 513);
    assert_eq!(
        manifest.parcels,
        (-3..497).map(|i| format!("{i},0")).collect::<Vec<_>>()
    );
    assert_eq!(manifest.spawn_coordinates.as_deref(), Some("-2,0"));
    for name in [empty.clone(), world_name()] {
        assert!(
            store::world_manifest(&f.pool, &name)
                .await
                .unwrap()
                .is_none()
        );
    }
    sqlx::query("UPDATE worlds.worlds SET spawn_coordinates = NULL WHERE name = $1")
        .bind(mixed.key())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        store::world_manifest(&f.pool, &mixed)
            .await
            .unwrap()
            .unwrap()
            .spawn_coordinates
            .is_none()
    );
    let body = expect(
        &f.app,
        Case::new("get", format!("/world/{}/manifest", mixed.display())),
        200,
    )
    .await;
    assert_eq!(body["total"], 513);
    assert_eq!(body["spawn_coordinate"], json!({ "x": "0", "y": "0" }));
}

#[tokio::test]
async fn every_route_answers_its_contract() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let owner = Wallet::random();
    let stranger = Wallet::random();
    let outsider = Wallet::random();
    f.names.set(&owner.address, world.label());
    let app = &f.app;
    let w = world.display();

    let thumb = vec![0u8, 1, 2, 3, 4, 5, 6, 7];
    let good = deployment_with(
        &owner,
        &world,
        "Gate World",
        "0,0",
        &["0,0", "0,1"],
        "thumb.png",
        &thumb,
    )
    .await;
    let published = post_entity(app, &good, 200).await;
    assert!(
        published["message"]
            .as_str()
            .unwrap()
            .contains(&good.entity_id)
    );
    assert!(
        f.storage.exist(&good.thumb_hash).await.unwrap(),
        "the content file landed in the blob store"
    );
    assert!(
        f.storage.exist(&good.entity_id).await.unwrap(),
        "the entity file landed in the blob store"
    );
    let (junk, junk_type) = multipart_body(&[("something", None, b"else")]);
    expect(
        app,
        Case::new("post", "/entities").body(junk, &junk_type),
        400,
    )
    .await;
    let bad_thumb = deployment_with(
        &owner,
        &world,
        "Gate World",
        "0,0",
        &["0,0", "0,1"],
        "https://example.com/image.png",
        &thumb,
    )
    .await;
    post_entity(app, &bad_thumb, 400).await;
    let banned = world_name();
    f.state.names.seed(vec![banned.label().to_string()]);
    f.names.set(&owner.address, banned.label());
    let denied = deployment(&owner, &banned, "Denied", "0,0", &["0,0"]).await;
    let body = post_entity(app, &denied, 400).await;
    assert!(
        body["errors"][0]
            .as_str()
            .unwrap()
            .contains("name deny list")
    );
    f.state.names.seed(Vec::new());

    expect(app, Case::new("get", "/worlds"), 200).await;
    expect(app, Case::new("get", "/worlds?sort=bogus"), 400).await;
    let active = expect(
        app,
        Case::new("post", "/entities/active").json(&json!({ "pointers": [w] })),
        200,
    )
    .await;
    assert_eq!(active[0]["id"], good.entity_id);
    assert_eq!(active[0]["metadata"]["owner"], owner.address.as_str());
    expect(
        app,
        Case::new("post", "/entities/active").json(&json!({})),
        400,
    )
    .await;
    expect(app, Case::new("get", format!("/world/{w}/manifest")), 200).await;
    expect(app, Case::new("get", "/world/nope.dcl.eth/manifest"), 404).await;

    let settings = expect(app, Case::new("get", format!("/world/{w}/settings")), 200).await;
    assert_eq!(settings["title"], "Gate World");
    expect(app, Case::new("get", "/world/nope.dcl.eth/settings"), 404).await;
    let settings_path = format!("/world/{w}/settings");
    let (sbody, stype) = multipart_body(&[("title", None, b"Gate World Renamed")]);
    let updated = expect(
        app,
        Case::new("put", &settings_path)
            .signed(&owner)
            .await
            .body(sbody, &stype),
        200,
    )
    .await;
    assert_eq!(updated["settings"]["title"], "Gate World Renamed");
    assert_eq!(updated["settings"]["settings_version"], 1);
    let (sbody, stype) = multipart_body(&[("title", None, b"Gate World Renamed")]);
    expect(
        app,
        Case::new("put", &settings_path).body(sbody, &stype),
        400,
    )
    .await;
    let (sbody, stype) = multipart_body(&[("title", None, b"Stranger Title")]);
    expect(
        app,
        Case::new("put", &settings_path)
            .signed(&stranger)
            .await
            .body(sbody, &stype),
        403,
    )
    .await;
    let (big_body, big_type) = multipart_body(&[(
        "thumbnail",
        Some("thumbnail.png"),
        &vec![0u8; 2 * 1024 * 1024 + 1],
    )]);
    expect(
        app,
        Case::new("put", &settings_path)
            .signed(&owner)
            .await
            .body(big_body, &big_type),
        400,
    )
    .await;

    let perms_path = format!("/world/{w}/permissions");
    let perms = expect(app, Case::new("get", &perms_path), 200).await;
    assert_eq!(perms["owner"], owner.address.as_str());
    assert_eq!(
        perms["permissions"]["access"],
        json!({ "type": "unrestricted" })
    );
    let deploy_perm_path = format!("/world/{w}/permissions/deployment");
    let meta = json!({ "type": "allow-list", "wallets": [stranger.display()] }).to_string();
    expect(
        app,
        Case::new("post", &deploy_perm_path)
            .signed_meta(&owner, &meta)
            .await,
        204,
    )
    .await;
    expect(
        app,
        Case::new("post", format!("/world/{w}/permissions/bogus"))
            .signed_meta(&owner, &json!({ "type": "allow-list" }).to_string())
            .await,
        400,
    )
    .await;
    expect(
        app,
        Case::new("post", &deploy_perm_path)
            .signed_meta(&stranger, &meta)
            .await,
        403,
    )
    .await;

    let addr_path = format!("/world/{w}/permissions/streaming/{}", stranger.display());
    expect(app, Case::new("put", &addr_path).signed(&owner).await, 204).await;
    expect(
        app,
        Case::new("put", format!("/world/{w}/permissions/streaming/zzz"))
            .signed(&owner)
            .await,
        400,
    )
    .await;
    let perms = expect(app, Case::new("get", &perms_path), 200).await;
    assert_eq!(
        perms["permissions"]["streaming"]["wallets"],
        json!([stranger.address.as_str()])
    );
    assert_eq!(
        perms["summary"][stranger.address.as_str()]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let parcels_path = format!(
        "/world/{w}/permissions/deployment/address/{}/parcels",
        stranger.display()
    );
    expect(
        app,
        Case::new("post", &parcels_path)
            .signed(&owner)
            .await
            .json(&json!({ "parcels": ["0,0"] })),
        204,
    )
    .await;
    let bogus = expect(
        app,
        Case::new(
            "post",
            format!(
                "/world/{w}/permissions/bogus/address/{}/parcels",
                stranger.display()
            ),
        )
        .signed(&owner)
        .await
        .json(&json!({ "parcels": ["0,0"] })),
        400,
    )
    .await;
    assert_eq!(
        bogus["message"],
        "Permission 'bogus' does not support allow-list. Only 'deployment' and 'streaming' do."
    );
    let listed = expect(app, Case::new("get", &parcels_path), 200).await;
    assert_eq!(listed, json!({ "total": 1, "parcels": ["0,0"] }));
    expect(
        app,
        Case::new(
            "get",
            format!(
                "/world/{w}/permissions/streaming/address/{}/parcels",
                owner.display()
            ),
        ),
        404,
    )
    .await;
    let holders = expect(
        app,
        Case::new("post", format!("/world/{w}/permissions/deployment/parcels"))
            .json(&json!({ "parcels": ["0,0"] })),
        200,
    )
    .await;
    assert_eq!(
        holders,
        json!({ "total": 1, "addresses": [stranger.address.as_str()] })
    );
    expect(
        app,
        Case::new("post", format!("/world/{w}/permissions/bogus/parcels"))
            .json(&json!({ "parcels": ["0,0"] })),
        400,
    )
    .await;
    expect(
        app,
        Case::new("delete", &parcels_path)
            .signed(&owner)
            .await
            .json(&json!({ "parcels": ["0,0"] })),
        204,
    )
    .await;
    expect(app, Case::new("get", &parcels_path), 404).await;
    assert!(
        !f.state
            .policy
            .has_world_wide_permission(&world, Permission::Deployment, &stranger.address)
            .await
            .unwrap()
    );
    let bogus = expect(
        app,
        Case::new(
            "delete",
            format!(
                "/world/{w}/permissions/bogus/address/{}/parcels",
                stranger.display()
            ),
        )
        .signed(&owner)
        .await
        .json(&json!({ "parcels": ["0,0"] })),
        400,
    )
    .await;
    assert_eq!(
        bogus["message"],
        "Permission 'bogus' does not support allow-list. Only 'deployment' and 'streaming' do."
    );
    expect(
        app,
        Case::new("delete", &addr_path).signed(&owner).await,
        204,
    )
    .await;
    expect(
        app,
        Case::new("delete", format!("/world/{w}/permissions/streaming/zzz"))
            .signed(&owner)
            .await,
        400,
    )
    .await;

    let access_path = format!("/world/{w}/permissions/access");
    expect(
        app,
        Case::new("post", &access_path)
            .signed_meta(
                &owner,
                &json!({ "type": "allow-list", "wallets": [], "communities": [] }).to_string(),
            )
            .await,
        204,
    )
    .await;
    let community = seed_community(&f.pool, &[&owner.address]).await;
    let community_path = format!("/world/{w}/permissions/access/communities/{community}");
    expect(
        app,
        Case::new("put", &community_path).signed(&owner).await,
        204,
    )
    .await;
    expect(
        app,
        Case::new(
            "put",
            format!("/world/{w}/permissions/access/communities/%20"),
        )
        .signed(&owner)
        .await,
        400,
    )
    .await;
    let perms = expect(app, Case::new("get", &perms_path), 200).await;
    assert_eq!(
        perms["permissions"]["access"],
        json!({ "type": "allow-list", "wallets": [], "communities": [community.to_string()] })
    );
    expect(
        app,
        Case::new("delete", &community_path).signed(&owner).await,
        204,
    )
    .await;
    expect(
        app,
        Case::new(
            "delete",
            format!("/world/{w}/permissions/access/communities/%20"),
        )
        .signed(&owner)
        .await,
        400,
    )
    .await;
    expect(
        app,
        Case::new("post", &access_path)
            .signed_meta(
                &owner,
                &json!({ "type": "shared-secret", "secret": "MySecret" }).to_string(),
            )
            .await,
        204,
    )
    .await;
    let now = chrono::Utc::now().naive_utc();
    assert_eq!(
        f.state
            .policy
            .can_access(&world, &stranger.address, Some("MySecret"), None, None, now)
            .await
            .unwrap(),
        crate::policy::Access::Allowed(crate::policy::AccessGrant::SharedSecret)
    );
    let perms = expect(app, Case::new("get", &perms_path), 200).await;
    assert_eq!(
        perms["permissions"]["access"],
        json!({ "type": "shared-secret" }),
        "the secret never leaves"
    );

    use crate::comms::gatekeeper::events::RoomEventSink;
    let room = crate::comms::rooms::worlds_service_world_room(&world);
    let join_body = serde_json::to_vec(&json!({ "event": "participant_joined", "room": { "name": room }, "participant": { "identity": owner.display() } })).unwrap();
    let _ = join_body;
    use crate::comms::protocol::bus::room_events::{RoomEvent, RoomEventKind, RoomType};
    let event = RoomEvent {
        kind: RoomEventKind::Joined as i32,
        room: room.clone(),
        room_type: RoomType::World as i32,
        realm_name: world.key().to_owned(),
        identity: owner.display(),
        ..Default::default()
    };
    f.state.observe_room_event(&event);
    f.state.observe_room_event(&RoomEvent {
        kind: RoomEventKind::Unspecified as i32,
        ..event
    });
    assert_eq!(
        f.state.presence.world_participant_count(world.key()),
        0,
        "the live presence is the gatekeeper's webhook to feed"
    );
    assert_eq!(f.state.flush_access_log().await.unwrap(), 1);

    expect(
        app,
        Case::new("get", "/admin/worlds").bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    expect(app, Case::new("get", "/admin/worlds"), 403).await;
    expect(app, Case::new("post", "/admin/gc?dry_run=true"), 403).await;
    let gc = expect(
        app,
        Case::new("post", "/gc?dry_run=true").bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    assert_eq!(
        (gc["dryRun"].as_bool(), gc["removed"].as_u64()),
        (Some(true), Some(0)),
        "{gc}"
    );
    assert!(
        gc["minAgeSeconds"].as_u64().unwrap() >= 3600,
        "the floor holds"
    );
    let detail = expect(
        app,
        Case::new("get", format!("/admin/worlds/{w}")).bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    assert_eq!(detail["sceneCount"], 1);
    assert_eq!(detail["owner"], owner.address.as_str());
    expect(
        app,
        Case::new("get", "/admin/worlds/nope.dcl.eth").bearer(ADMIN_TOKEN),
        404,
    )
    .await;
    let ban = expect(
        app,
        Case::new(
            "get",
            format!(
                "/admin/worlds/{w}/ban-status?address={}&parcel=0,0",
                owner.display()
            ),
        )
        .bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    assert_eq!(
        (
            ban["platformBanned"].as_bool(),
            ban["sceneBanned"].as_bool()
        ),
        (Some(false), Some(false))
    );
    expect(
        app,
        Case::new(
            "get",
            format!("/admin/worlds/{w}/ban-status?address={}", owner.display()),
        ),
        403,
    )
    .await;
    expect(
        app,
        Case::new("post", format!("/admin/worlds/{w}/disable")).bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    expect(
        app,
        Case::new("post", "/admin/worlds/nope.dcl.eth/disable").bearer(ADMIN_TOKEN),
        404,
    )
    .await;
    expect(
        app,
        Case::new("post", format!("/admin/worlds/{w}/enable")).bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    expect(
        app,
        Case::new("post", "/admin/worlds/nope.dcl.eth/enable").bearer(ADMIN_TOKEN),
        404,
    )
    .await;
    let blocked_path = format!("/admin/blocked/{}", stranger.display());
    expect(
        app,
        Case::new("post", &blocked_path).bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    expect(app, Case::new("post", &blocked_path), 403).await;
    assert_eq!(
        f.state
            .policy
            .is_banned(&stranger.address, None, None, now)
            .await
            .unwrap(),
        Some(crate::policy::Ban::Blocked),
        "a blocked wallet is refused every world join"
    );
    let ban = expect(
        app,
        Case::new(
            "get",
            format!(
                "/admin/worlds/{w}/ban-status?address={}",
                stranger.display()
            ),
        )
        .bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    assert_eq!(ban["platformBanned"], true);
    let listed = expect(
        app,
        Case::new("get", "/admin/blocked").bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    assert!(
        listed["blocked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["wallet"] == stranger.address.as_str())
    );
    expect(app, Case::new("get", "/admin/blocked"), 403).await;
    expect(
        app,
        Case::new("delete", &blocked_path).bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    expect(
        app,
        Case::new("delete", &blocked_path).bearer(ADMIN_TOKEN),
        404,
    )
    .await;
    let log = expect(
        app,
        Case::new("get", format!("/admin/access-log?world={w}")).bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    assert_eq!(log["entries"][0]["action"], "join");
    assert_eq!(log["entries"][0]["address"], owner.address.as_str());
    let log = expect(
        app,
        Case::new(
            "get",
            format!("/admin/access-log?address={}", owner.display()),
        )
        .bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    assert_eq!(log["entries"][0]["worldName"], world.key());
    expect(app, Case::new("get", "/admin/access-log"), 403).await;

    expect(
        app,
        Case::new("delete", format!("/world/{w}/scenes/0,0"))
            .signed(&outsider)
            .await,
        403,
    )
    .await;
    expect(
        app,
        Case::new("delete", format!("/world/{w}/scenes/0,0"))
            .signed(&owner)
            .await,
        200,
    )
    .await;
    expect(
        app,
        Case::new("delete", format!("/world/{w}/scenes/0,0"))
            .signed(&owner)
            .await,
        404,
    )
    .await;
    post_entity(app, &good, 200).await;
    expect(
        app,
        Case::new("delete", format!("/entities/{w}"))
            .signed(&outsider)
            .await,
        403,
    )
    .await;
    expect(
        app,
        Case::new("delete", format!("/entities/{w}"))
            .signed(&owner)
            .await,
        200,
    )
    .await;
    assert!(deployed_entity_ids(&f.pool, &world).await.is_empty());
}

const NEST: &str = "/worlds-content-server";

/// A request to `route` on the `worlds-content-server.<domain>` host: the
/// gateway nests it under [`NEST`] and forwards the client's path as
/// `x-original-path`; the signature is over `signed`.
async fn via_gateway(
    method: &'static str,
    route: &str,
    w: &Wallet,
    signed: &str,
    forwarded: Option<&str>,
    metadata: &str,
) -> Case {
    let case = Case::new(method, format!("{NEST}{route}"))
        .signed_over(w, signed, metadata)
        .await;
    match forwarded {
        Some(path) => case.header(ORIGINAL_PATH_HEADER, path),
        None => case,
    }
}

/// Every signed route family through the nest: a signature over the bare
/// path verifies with the gateway's `x-original-path`, one over the prefixed
/// path still verifies, and one for another route is refused.
#[tokio::test]
async fn signed_routes_verify_the_bare_path_through_the_gateway_nest() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let owner = Wallet::random();
    f.names.set(&owner.address, world.label());
    let w = world.display();
    let app = &Router::new().nest(NEST, f.app.clone());
    let scene = deployment(&owner, &world, "Nest World", "0,0", &["0,0"]).await;
    post_entity(&f.app, &scene, 200).await;

    let settings = format!("/world/{w}/settings");
    let access = format!("/world/{w}/permissions/access");
    let grants = format!("/world/{w}/permissions/deployment");
    let scene_path = format!("/world/{w}/scenes/0,0");
    let world_path = format!("/entities/{w}");
    let prefixed = |route: &str| format!("{NEST}{route}");

    for (signed, forwarded, status) in [
        (settings.clone(), Some(settings.clone()), 200),
        (prefixed(&settings), None, 200),
        (access.clone(), Some(access.clone()), 401),
        (settings.clone(), None, 401),
    ] {
        let (body, ty) = multipart_body(&[("title", None, b"Nest World Renamed")]);
        let case = via_gateway(
            "put",
            &settings,
            &owner,
            &signed,
            forwarded.as_deref(),
            "{}",
        )
        .await;
        expect(app, case.body(body, &ty), status).await;
    }

    let grant = json!({ "type": "allow-list", "wallets": [] }).to_string();
    for (signed, forwarded, status) in [
        (grants.clone(), Some(grants.clone()), 204),
        (prefixed(&grants), None, 204),
        (access.clone(), Some(access.clone()), 401),
    ] {
        let case = via_gateway(
            "post",
            &grants,
            &owner,
            &signed,
            forwarded.as_deref(),
            &grant,
        )
        .await;
        expect(app, case, status).await;
    }

    let mut deployed = true;
    for (route, signed, forwarded, status) in [
        (
            &scene_path,
            world_path.clone(),
            Some(world_path.clone()),
            401,
        ),
        (
            &scene_path,
            scene_path.clone(),
            Some(scene_path.clone()),
            200,
        ),
        (&scene_path, prefixed(&scene_path), None, 200),
        (
            &world_path,
            scene_path.clone(),
            Some(scene_path.clone()),
            401,
        ),
        (
            &world_path,
            world_path.clone(),
            Some(world_path.clone()),
            200,
        ),
        (&world_path, prefixed(&world_path), None, 200),
    ] {
        if !deployed {
            post_entity(&f.app, &scene, 200).await;
        }
        let case = via_gateway("delete", route, &owner, &signed, forwarded.as_deref(), "{}").await;
        expect(app, case, status).await;
        deployed = status != 200;
    }
    assert!(deployed_entity_ids(&f.pool, &world).await.is_empty());
}

/// The owner-gated permission routes (`verify_owner`) through the nest: a
/// bare-path signature with `x-original-path` passes, another route's fails.
#[tokio::test]
async fn owner_gated_permission_routes_verify_the_bare_path_through_the_gateway_nest() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let owner = Wallet::random();
    let grantee = Wallet::random();
    f.names.set(&owner.address, world.label());
    let w = world.display();
    let app = &Router::new().nest(NEST, f.app.clone());
    let scene = deployment(&owner, &world, "Nest World", "0,0", &["0,0"]).await;
    post_entity(&f.app, &scene, 200).await;
    let access = format!("/world/{w}/permissions/access");
    expect(
        &f.app,
        Case::new("post", &access)
            .signed_meta(
                &owner,
                &json!({ "type": "allow-list", "wallets": [], "communities": [] }).to_string(),
            )
            .await,
        204,
    )
    .await;

    let address = format!("/world/{w}/permissions/streaming/{}", grantee.display());
    let parcels = format!(
        "/world/{w}/permissions/deployment/address/{}/parcels",
        grantee.display()
    );
    // Adding a community requires the signer to be one of its members.
    let community = format!(
        "/world/{w}/permissions/access/communities/{}",
        seed_community(&f.pool, &[&owner.address]).await
    );
    let body = json!({ "parcels": ["0,0"] });
    let steps: [(&'static str, &String, Option<&Value>); 6] = [
        ("put", &address, None),
        ("post", &parcels, Some(&body)),
        ("delete", &parcels, Some(&body)),
        ("put", &community, None),
        ("delete", &community, None),
        ("delete", &address, None),
    ];
    for (method, route, json) in steps {
        for (signed, status) in [(&access, 401), (route, 204)] {
            let case = via_gateway(method, route, &owner, signed, Some(signed), "{}").await;
            let case = match json {
                Some(v) => case.json(v),
                None => case,
            };
            expect(app, case, status).await;
        }
    }
    let perms = expect(
        &f.app,
        Case::new("get", format!("/world/{w}/permissions")),
        200,
    )
    .await;
    assert_eq!(
        perms["permissions"]["access"],
        json!({ "type": "allow-list", "wallets": [], "communities": [] })
    );
    assert_eq!(perms["permissions"]["streaming"]["wallets"], json!([]));
}

/// A local publish is queued in its transaction and sent, unchanged, to the
/// upstream's `POST /entities` with only the files it lacks; a scene
/// replaced before its send is superseded, never sent (the upstream applies
/// the last publish it receives); a refused publish is never queued.
#[tokio::test]
async fn local_publishes_are_forwarded_unchanged_and_replaced_ones_superseded() {
    use crate::auth::{AuthChain, verify::verify_auth_chain_async};
    use crate::content::create_entity::extract_auth_chain_from_fields;
    use tokio_util::sync::CancellationToken;

    let upstream = forward::test_support::FakeUpstream::start("").await;
    let Some(f) = fixture_with(PersonalWorldsPolicy::disabled(), Some(&upstream.base)).await else {
        return;
    };
    let forwarder = f.state.forwarder.clone().unwrap();
    let world = world_name();
    let owner = Wallet::random();
    let stranger = Wallet::random();
    f.names.set(&owner.address, world.label());

    let refused = deployment(&stranger, &world, "Stranger", "0,0", &["0,0"]).await;
    let (status, _) = hit(
        &f.app,
        Case::new("post", "/entities").body(refused.body.clone(), &refused.content_type),
    )
    .await;
    assert_ne!(status, StatusCode::OK);
    let first = deployment(&owner, &world, "First", "0,0", &["0,0"]).await;
    post_entity(&f.app, &first, 200).await;
    let second = deployment(&owner, &world, "Second", "0,0", &["0,0"]).await;
    post_entity(&f.app, &second, 200).await;
    let other = deployment(&owner, &world, "Other", "1,0", &["1,0"]).await;
    upstream.hold(&other.thumb_hash);
    post_entity(&f.app, &other, 200).await;

    let ids = [
        refused.entity_id.clone(),
        first.entity_id.clone(),
        second.entity_id.clone(),
        other.entity_id.clone(),
    ];
    let queued: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT entity_id, world_name, status FROM worlds.forward_outbox WHERE entity_id = ANY($1) ORDER BY id",
    )
    .bind(&ids[..])
    .fetch_all(&f.pool)
    .await
    .unwrap();
    let key = world.key().to_string();
    assert_eq!(
        queued,
        [&first, &second, &other]
            .iter()
            .map(|d| (d.entity_id.clone(), key.clone(), "queued".to_string()))
            .collect::<Vec<_>>()
    );

    let report = forwarder.run_pass(&CancellationToken::new()).await.unwrap();
    assert_eq!(
        (report.accepted, report.superseded, report.total()),
        (2, 1, 3)
    );

    let posts = upstream.posts();
    assert_eq!(posts.len(), 2);
    for (post, d, lacking) in [(&posts[0], &second, true), (&posts[1], &other, false)] {
        assert_eq!(post.content_length, Some(post.body.len() as u64));
        let form = forward::test_support::parse(post).await;
        assert_eq!(form.fields["entityId"], d.entity_id);
        let sent: AuthChain = serde_json::from_value(
            extract_auth_chain_from_fields(&form.fields)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        verify_auth_chain_async(&sent, &d.entity_id, None, None)
            .await
            .expect("the forwarded chain signs the forwarded entity");
        let stored: Value = sqlx::query_scalar(
            "SELECT deployment_auth_chain FROM worlds.world_scenes WHERE entity_id = $1",
        )
        .bind(&d.entity_id)
        .fetch_one(&f.pool)
        .await
        .unwrap();
        let stored: AuthChain = serde_json::from_value(stored).unwrap();
        assert_eq!(sent.len(), stored.len());
        for (sent, stored) in sent.iter().zip(&stored) {
            assert_eq!(
                (&sent.link_type, &sent.payload),
                (&stored.link_type, &stored.payload)
            );
            assert_eq!(
                sent.signature.as_deref().unwrap_or_default(),
                stored.signature.as_deref().unwrap_or_default()
            );
        }
        let entity = f.storage.retrieve(&d.entity_id).await.unwrap().unwrap();
        assert_eq!(
            hash_bytes_v1(&entity),
            d.entity_id,
            "the client's entity bytes"
        );
        let mut expected = vec![(d.entity_id.clone(), entity)];
        if lacking {
            let thumb = f.storage.retrieve(&d.thumb_hash).await.unwrap().unwrap();
            expected.push((d.thumb_hash.clone(), thumb));
        }
        assert_eq!(form.files, expected);
    }

    let record = expect(
        &f.app,
        Case::new("get", format!("/admin/forwarding/{}", first.entity_id)).bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    assert_eq!(record["status"], json!("superseded"));
    assert_eq!(record["worldName"], json!(key));
    assert_eq!(record["target"], json!(upstream.base));
    expect(
        &f.app,
        Case::new("get", format!("/admin/forwarding/{}", first.entity_id)),
        403,
    )
    .await;
    expect(
        &f.app,
        Case::new("get", format!("/admin/forwarding/{}", refused.entity_id)).bearer(ADMIN_TOKEN),
        404,
    )
    .await;
    let page = expect(
        &f.app,
        Case::new("get", "/admin/forwarding?status=accepted&limit=1000").bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    let listed: Vec<&Value> = page["forwards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["entityId"])
        .collect();
    assert!(listed.contains(&&json!(second.entity_id)));
    assert!(listed.contains(&&json!(other.entity_id)));
    expect(
        &f.app,
        Case::new("get", "/admin/forwarding?status=bogus").bearer(ADMIN_TOKEN),
        400,
    )
    .await;

    // A re-publish of an accepted scene is a fresh record, queued again.
    sqlx::query("DELETE FROM worlds.world_scenes WHERE entity_id = $1")
        .bind(&other.entity_id)
        .execute(&f.pool)
        .await
        .unwrap();
    post_entity(&f.app, &other, 200).await;
    let again: (String, i32) =
        sqlx::query_as("SELECT status, attempts FROM worlds.forward_outbox WHERE entity_id = $1")
            .bind(&other.entity_id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(again, ("queued".to_string(), 0));

    // The upstream refuses it; the operator requeues it once the cause is
    // fixed. A superseded record is not requeued: the scene is gone here.
    upstream.answer(400, "refused upstream");
    let report = forwarder.run_pass(&CancellationToken::new()).await.unwrap();
    assert_eq!((report.rejected, report.total()), (1, 1));
    let retry = format!("/admin/forwarding/{}/retry", other.entity_id);
    expect(&f.app, Case::new("post", retry.clone()), 403).await;
    let record = expect(&f.app, Case::new("post", retry).bearer(ADMIN_TOKEN), 200).await;
    assert_eq!(
        (&record["status"], &record["attempts"], &record["worldName"]),
        (&json!("queued"), &json!(0), &json!(key))
    );
    let report = forwarder.run_pass(&CancellationToken::new()).await.unwrap();
    assert_eq!((report.accepted, report.total()), (1, 1));
    expect(
        &f.app,
        Case::new(
            "post",
            format!("/admin/forwarding/{}/retry", first.entity_id),
        )
        .bearer(ADMIN_TOKEN),
        409,
    )
    .await;
    expect(
        &f.app,
        Case::new(
            "post",
            format!("/admin/forwarding/{}/retry", refused.entity_id),
        )
        .bearer(ADMIN_TOKEN),
        404,
    )
    .await;

    sqlx::query("DELETE FROM worlds.forward_outbox WHERE entity_id = ANY($1)")
        .bind(&ids[..])
        .execute(&f.pool)
        .await
        .unwrap();
}

/// A publish the mirror took over before its send (the scene row now has a
/// `mirror_source`) is no longer this node's: superseded, never sent. The
/// requeue route answers 503 while no upstream is named, and the lane keeps
/// a worlds server's window (`DEPLOYMENT_TTL`, five minutes).
#[tokio::test]
async fn a_publish_mirrored_over_before_its_send_is_superseded() {
    use tokio_util::sync::CancellationToken;

    assert_eq!(
        (
            super::forwarding::LANE.max_age_ms,
            super::forwarding::LANE.max_ahead_ms
        ),
        (5 * 60_000, 15 * 60_000)
    );
    let upstream = forward::test_support::FakeUpstream::start("").await;
    let Some(f) = fixture_with(PersonalWorldsPolicy::disabled(), Some(&upstream.base)).await else {
        return;
    };
    let forwarder = f.state.forwarder.clone().unwrap();
    let world = world_name();
    let owner = Wallet::random();
    f.names.set(&owner.address, world.label());
    let d = deployment(&owner, &world, "Mirrored", "0,0", &["0,0"]).await;
    post_entity(&f.app, &d, 200).await;
    sqlx::query(
        "UPDATE worlds.world_scenes SET mirror_source = 'https://mirror.example' WHERE entity_id = $1",
    )
    .bind(&d.entity_id)
    .execute(&f.pool)
    .await
    .unwrap();

    let report = forwarder.run_pass(&CancellationToken::new()).await.unwrap();
    assert_eq!((report.superseded, report.total()), (1, 1));
    assert!(upstream.posts().is_empty() && upstream.probed().is_empty());
    let record = expect(
        &f.app,
        Case::new("get", format!("/admin/forwarding/{}", d.entity_id)).bearer(ADMIN_TOKEN),
        200,
    )
    .await;
    assert_eq!(record["status"], json!("superseded"));

    let off = fixture().await.unwrap();
    expect(
        &off.app,
        Case::new("post", format!("/admin/forwarding/{}/retry", d.entity_id)).bearer(ADMIN_TOKEN),
        503,
    )
    .await;

    sqlx::query("DELETE FROM worlds.forward_outbox WHERE entity_id = $1")
        .bind(&d.entity_id)
        .execute(&f.pool)
        .await
        .unwrap();
}

/// A world deployed by `owner`, who also holds its NAME.
async fn owned_world(f: &Fixture, owner: &Wallet) -> WorldName {
    let world = world_name();
    f.names.set(&owner.address, world.label());
    let fp = ["0,0", "0,1"];
    deploy(
        f,
        &world,
        &owner.address,
        &format!("bafy{}", Uuid::new_v4().simple()),
        &scene_entity(&world, "Owned World", "bafymissing", "0,0", &fp),
        &fp,
        &Deploy::Owner,
    )
    .await
    .unwrap();
    world
}

async fn put_settings(
    f: &Fixture,
    world: &WorldName,
    w: &Wallet,
    parts: &[(&str, Option<&str>, &[u8])],
    status: u16,
) -> Value {
    let (body, content_type) = multipart_body(parts);
    expect(
        &f.app,
        Case::new("put", format!("/world/{}/settings", world.display()))
            .signed(w)
            .await
            .body(body, &content_type),
        status,
    )
    .await
}

#[tokio::test]
async fn a_settings_thumbnail_is_stored_only_for_an_authorized_writer() {
    let Some(f) = fixture().await else { return };
    let (owner, stranger) = (Wallet::random(), Wallet::random());
    let world = owned_world(&f, &owner).await;
    let mut png = PNG_MAGIC.to_vec();
    png.extend_from_slice(Uuid::new_v4().as_bytes());
    let hash = hash_bytes_v1(&png);
    let parts: [(&str, Option<&str>, &[u8]); 1] = [("thumbnail", Some("t.png"), &png)];

    put_settings(&f, &world, &stranger, &parts, 403).await;
    assert!(
        !f.storage.exist(&hash).await.unwrap(),
        "an unauthorized PUT stores nothing"
    );

    let updated = put_settings(&f, &world, &owner, &parts, 200).await;
    assert_eq!(updated["settings"]["thumbnail_hash"], json!(hash));
    assert!(f.storage.exist(&hash).await.unwrap());
}

#[tokio::test]
async fn settings_empty_values_follow_the_upstream_handler() {
    let Some(f) = fixture().await else { return };
    let owner = Wallet::random();
    let world = owned_world(&f, &owner).await;

    let body = put_settings(&f, &world, &owner, &[("title", None, b"")], 400).await;
    assert_eq!(
        body["message"],
        "Invalid title: title cannot be empty. Expected between 3 and 100 characters."
    );
    let body = put_settings(&f, &world, &owner, &[("description", None, b"")], 400).await;
    assert_eq!(
        body["message"],
        "Invalid description: description cannot be empty. Expected between 3 and 1000 characters."
    );

    put_settings(
        &f,
        &world,
        &owner,
        &[
            ("categories", None, b"art"),
            ("single_player", None, b"true"),
            ("show_in_places", None, b"true"),
            ("content_rating", None, b"T"),
        ],
        200,
    )
    .await;
    let body = put_settings(
        &f,
        &world,
        &owner,
        &[
            ("categories", None, b"game"),
            ("categories", None, b""),
            ("single_player", None, b""),
            ("show_in_places", None, b""),
            ("spawn_coordinates", None, b""),
            ("content_rating", None, b""),
        ],
        200,
    )
    .await;
    let s = &body["settings"];
    assert_eq!(
        s["categories"],
        json!(["art"]),
        "an empty category drops the field"
    );
    assert_eq!(s["single_player"], json!(true));
    assert_eq!(s["show_in_places"], json!(true));
    assert_eq!(s["spawn_coordinates"], json!("0,0"));
    assert_eq!(s["content_rating"], json!("T"));
}

#[tokio::test]
async fn parcel_bodies_are_schema_validated_on_every_parcel_route() {
    let Some(f) = fixture().await else { return };
    let owner = Wallet::random();
    let grantee = Wallet::random();
    let world = owned_world(&f, &owner).await;
    let w = world.display();
    let address_path = format!(
        "/world/{w}/permissions/deployment/address/{}/parcels",
        grantee.display()
    );
    let holders_path = format!("/world/{w}/permissions/deployment/parcels");
    let too_many: Vec<String> = (0..501).map(|i| format!("{i},0")).collect();
    let bad_bodies = [
        json!({ "parcels": [] }),
        json!({ "parcels": too_many }),
        json!({ "parcels": ["a,b"] }),
        json!({ "parcels": ["0,0"], "extra": 1 }),
        json!({}),
    ];

    for body in &bad_bodies {
        for (method, path, signed) in [
            ("post", &address_path, true),
            ("delete", &address_path, true),
            ("post", &holders_path, false),
        ] {
            let mut case = Case::new(method, path.as_str());
            if signed {
                case = case.signed(&owner).await;
            }
            let answer = expect(&f.app, case.json(body), 400).await;
            assert_eq!(
                answer["message"], "Invalid JSON body",
                "{method} {path} {body}"
            );
        }
    }
    expect(
        &f.app,
        Case::new("post", holders_path.as_str())
            .body(b"{\"parcels\":[\"0,0\"]}".to_vec(), "text/plain"),
        415,
    )
    .await;
    assert!(
        store::permission_records(&f.pool, &world)
            .await
            .unwrap()
            .is_empty(),
        "no rejected body granted anything"
    );

    let max: Vec<String> = (0..500)
        .map(|i| format!("{},{}", i % 100, i / 100))
        .collect();
    expect(
        &f.app,
        Case::new("post", address_path.as_str())
            .signed(&owner)
            .await
            .json(&json!({ "parcels": max })),
        204,
    )
    .await;
}

#[tokio::test]
async fn parcel_listings_paginate_like_http_commons() {
    let Some(f) = fixture().await else { return };
    let owner = Wallet::random();
    let grantee = Wallet::random();
    let world = owned_world(&f, &owner).await;
    let w = world.display();
    let path = format!(
        "/world/{w}/permissions/streaming/address/{}/parcels",
        grantee.display()
    );
    let granted: Vec<String> = (0..150).map(|i| format!("{i},0")).collect();
    expect(
        &f.app,
        Case::new("post", path.as_str())
            .signed(&owner)
            .await
            .json(&json!({ "parcels": granted })),
        204,
    )
    .await;

    for (query, expected) in [
        ("", 100),
        ("?limit=1000", 100),
        ("?limit=abc", 100),
        ("?limit=0", 100),
        ("?limit=10&offset=-5", 10),
        ("?limit=10&offset=145", 5),
    ] {
        let page = expect(&f.app, Case::new("get", format!("{path}{query}")), 200).await;
        assert_eq!(page["total"], 150, "{query}");
        assert_eq!(
            page["parcels"].as_array().unwrap().len(),
            expected,
            "{query}"
        );
    }
    let boxed = expect(
        &f.app,
        Case::new("get", format!("{path}?x1=0&y1=0&x2=1.5&y2=0")),
        200,
    )
    .await;
    assert_eq!(boxed["parcels"], json!(["0,0", "1,0"]));
    let bad = expect(
        &f.app,
        Case::new("get", format!("{path}?x1=a&y1=0&x2=1&y2=0")),
        400,
    )
    .await;
    assert_eq!(
        bad["message"],
        "Bounding box parameters must be valid integers."
    );

    let holders = expect(
        &f.app,
        Case::new(
            "post",
            format!("/world/{w}/permissions/streaming/parcels?limit=1000&offset=x"),
        )
        .json(&json!({ "parcels": ["0,0"] })),
        200,
    )
    .await;
    assert_eq!(holders["addresses"], json!([grantee.address.as_str()]));
}

#[tokio::test]
async fn deleting_an_address_grant_requires_a_valid_world() {
    let Some(f) = fixture().await else { return };
    let owner = Wallet::random();
    let grantee = Wallet::random();
    let delete = |world: &WorldName| {
        Case::new(
            "delete",
            format!(
                "/world/{}/permissions/deployment/{}",
                world.display(),
                grantee.display()
            ),
        )
    };

    let (sceneless, sceneless_owner) = (world_name(), Wallet::random());
    f.names.set(&sceneless_owner.address, sceneless.label());
    store::ensure_world(&f.pool, &sceneless, &sceneless_owner.address)
        .await
        .unwrap();
    expect(
        &f.app,
        delete(&sceneless).signed(&sceneless_owner).await,
        404,
    )
    .await;

    let world = owned_world(&f, &owner).await;
    assert!(
        store::admin_set_world_blocked(&f.pool, &world, true)
            .await
            .unwrap()
    );
    let body = expect(&f.app, delete(&world).signed(&owner).await, 404).await;
    assert_eq!(
        body["message"],
        json!(format!("World \"{}\" not found.", world.display()))
    );
    assert!(
        store::admin_set_world_blocked(&f.pool, &world, false)
            .await
            .unwrap()
    );
    expect(&f.app, delete(&world).signed(&owner).await, 204).await;
}

/// The whole worlds router as `serve` mounts it, over the fixture's pool and
/// blob store.
async fn served_app(f: &Fixture) -> Router {
    let worlds = Arc::new(crate::WorldsState::new(
        f.pool.clone(),
        f.storage.clone(),
        crate::Settings::for_tests("https://worlds.example.org"),
        None,
        crate::http::Outbound::for_tests(),
    ));
    let blobs = Arc::new(crate::content::ContentState::new(
        f.pool.clone(),
        crate::content::Settings::for_tests(),
        f.storage.clone(),
    ));
    let stage5 = crate::Stage5 {
        core: f.state.clone(),
        storage: Arc::new(crate::storage::StorageState::new(
            f.pool.clone(),
            f.state.policy.clone(),
            crate::storage::Settings::for_tests(None),
            None,
        )),
        scene_host: None,
        fed: crate::fed::FedState::boot(f.pool.clone(), Default::default(), None)
            .await
            .unwrap(),
        writes_enabled: true,
    };
    crate::app_router(worlds, blobs, &stage5)
}

/// The deploy clients ask the worlds host which files it already holds
/// before `POST /entities`; a file a world deploy stored is reported held.
#[tokio::test]
async fn available_content_under_the_worlds_prefix_reports_deployed_files() {
    let Some(f) = fixture().await else { return };
    let world = world_name();
    let owner = Wallet::random();
    f.names.set(&owner.address, world.label());
    let d = deployment(&owner, &world, "Held World", "0,0", &["0,0"]).await;
    post_entity(&f.app, &d, 200).await;
    let missing = hash_bytes_v1(Uuid::new_v4().as_bytes());
    let app = served_app(&f).await;

    let body = expect(
        &app,
        Case::new(
            "get",
            format!(
                "/worlds-content-server/available-content?cid={}&cid={missing}",
                d.thumb_hash
            ),
        ),
        200,
    )
    .await;
    assert_eq!(
        body,
        json!([
            { "cid": d.thumb_hash, "available": true },
            { "cid": missing, "available": false },
        ])
    );

    let body = expect(
        &app,
        Case::new("get", "/worlds-content-server/available-content"),
        400,
    )
    .await;
    assert_eq!(
        body,
        json!({ "error": "Bad request", "message": "At least one cid query parameter is required." })
    );

    let body = expect(
        &app,
        Case::new(
            "get",
            format!(
                "/worlds-content-server/available-content?cid={}&cid=bafkreinotacid",
                d.thumb_hash
            ),
        ),
        400,
    )
    .await;
    assert_eq!(
        body,
        json!({ "error": "Bad request", "message": "Invalid cid format." })
    );

    let too_many = vec![format!("cid={missing}"); 501].join("&");
    let body = expect(
        &app,
        Case::new(
            "get",
            format!("/worlds-content-server/available-content?{too_many}"),
        ),
        400,
    )
    .await;
    assert_eq!(
        body,
        json!({ "error": "Bad request", "message": "Too many cid query parameters. Maximum allowed is 500." })
    );

    let preflight = app
        .clone()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/worlds-content-server/available-content")
                .header(header::ORIGIN, "https://creator.example.org")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        preflight.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://creator.example.org"
    );
}
