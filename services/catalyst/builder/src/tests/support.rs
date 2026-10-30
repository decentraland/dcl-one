// from: tests/support/db.rs (the scratch database), catalyrst/crates/catalyrst-builder/tests/*.rs (each old file built its own pool, store, auth chain and cleanup; shared here)
//! Shared fixtures of the builder's copied contract tests: the process's
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

pub async fn temp_storage() -> Arc<Storage> {
    let dir = std::env::temp_dir().join(format!("catalyst-builder-test-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    Arc::new(Storage::from_config(&dir).await.unwrap())
}

/// `None` (skip) without `CATALYST_TEST_DATABASE_URL`.
pub async fn scratch_component() -> Option<ItemsComponent> {
    let pool = crate::db::scratch::pool(6).await?;
    Some(ItemsComponent::new(pool, temp_storage().await))
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

fn lazy_pool() -> sqlx::PgPool {
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

async fn state_over_with(
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

/// `None` (skip) without `CATALYST_TEST_DATABASE_URL`.
pub async fn state_with_db(
    market: Option<Arc<crate::chain_index::ChainIndex>>,
) -> Option<crate::SharedState> {
    let pool = crate::db::scratch::pool(4).await?;
    Some(state_over(pool, market).await)
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
