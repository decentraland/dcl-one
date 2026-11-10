//! The validator over an entity a peer serves (sync, the failed-deployments
//! retries, the read-through): every rule [`super::validate`] runs, minus the
//! server-side checks the old node skipped for synced deployments (request
//! recency, rate limits, newer entity on the pointers, which live in the
//! write deployer and never reach the validator).
//!
//! What differs from a local deployment:
//! - the files are the entity file alone; content files are read from the
//!   store sync downloaded them into.
//!   [`SyncedContent::Pending`] (the read-through's check before it stores
//!   anything) counts every content file as present.
//! - the auth chain: expiry against the entity timestamp, an expiration that
//!   parses as no known date kept as the peer's verdict, a contract-wallet
//!   link asked of the node's EIP-1654 validator at the last block at or
//!   before the entity timestamp (none configured, or an RPC that cannot
//!   answer for that block: undecidable).
//! - items, as the reference validator: no thumbnail check, and pointers
//!   naming one collections-v1 asset by address and by name count as one.
//! - a timestamp more than `REQUEST_TTL_FORWARDS_MS` ahead of this node's
//!   clock is undecided until the clock catches up; more than a day ahead,
//!   refused.
//! - one deadline bounds the whole validation of an entity, its contract
//!   wallet included; past it the entity is undecided.
//! - access: every check asks the chain at the entity timestamp's block, then
//!   five minutes before it ([`super::onchain`]), as for a local deploy; item
//!   ownership leaves out the escrow lease overlay, which knows only the
//!   leases active now, and counts a collection URN naming no item as owned
//!   (the reference skips URNs it cannot parse). Its item caps are wider than
//!   a local deploy's and past them the entity is undecided, never refused
//!   (the reference has no cap). A source unset or failing, or a node without
//!   that block's state, is undecidable; a revert is a "no".

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;

use super::checker::{BlockchainChecker, BlockchainLayer};
use super::error::{PermissionResult, ValidationError, ValidationResponse, ValidatorError};
use super::rules::{ContentValidator, ExternalCalls};
use super::types::{Deployment, Entity, Timestamp};
use super::{Sources, entity_parser};
use crate::auth::verify::{ChainVerdict, verify_synced_auth_chain};
use crate::auth::{AuthChain, AuthError, Eip1654Validator};

/// Whether the entity's content files are in the store yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncedContent {
    /// Downloaded and hash-verified: the store answers for them.
    Fetched,
    /// Not downloaded yet: every referenced file counts as present.
    Pending,
}

struct Synced<'a> {
    sources: &'a Sources,
    content: SyncedContent,
    trusted_expirations: Mutex<Vec<String>>,
}

impl Synced<'_> {
    /// Whether every content file counts as present (with size 0) without asking the store.
    fn assumed_present(&self) -> bool {
        self.content == SyncedContent::Pending
    }
}

#[async_trait]
impl ExternalCalls for Synced<'_> {
    async fn is_content_stored_already(
        &self,
        hashes: &[String],
    ) -> Result<HashMap<String, bool>, String> {
        if self.assumed_present() {
            return Ok(hashes.iter().map(|h| (h.clone(), true)).collect());
        }
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        self.sources.is_content_stored_already(hashes).await
    }

    async fn fetch_content_file_size(&self, hash: &str) -> Result<Option<usize>, String> {
        if self.assumed_present() {
            return Ok(Some(0));
        }
        self.sources.fetch_content_file_size(hash).await
    }

    async fn validate_signature(
        &self,
        entity_id: &str,
        auth_chain: &AuthChain,
        timestamp: Timestamp,
    ) -> Result<(), AuthError> {
        let at_block = self.sources.eip1654().map(|inner| AtEntityBlock {
            inner,
            sources: self.sources,
            timestamp_ms: timestamp,
        });
        let verdict = verify_synced_auth_chain(
            auth_chain,
            entity_id,
            timestamp,
            at_block.as_ref().map(|v| v as &dyn Eip1654Validator),
        )
        .await?;
        if let Ok(mut trusted) = self.trusted_expirations.lock() {
            trusted.extend(verdict.trusted_expirations);
        }
        Ok(())
    }
}

/// The node's EIP-1654 validator asked at the last L1 block at or before the
/// entity timestamp, found by the search the access checks share.
struct AtEntityBlock<'a> {
    inner: &'a dyn Eip1654Validator,
    sources: &'a Sources,
    timestamp_ms: i64,
}

#[async_trait]
impl Eip1654Validator for AtEntityBlock<'_> {
    async fn validate_signature(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
    ) -> Result<bool, AuthError> {
        let block = self
            .sources
            .access
            .l1_block_at(self.timestamp_ms)
            .await
            .map_err(|e| AuthError::Eip1654ValidationFailed(e.to_string()))?;
        // Before the first block no contract could have signed.
        let Some(block) = block else {
            return Ok(false);
        };
        self.inner
            .validate_signature_at_block(contract_address, hash, signature, block)
            .await
    }
}

/// Every access question goes to the chain at the entity timestamp, as for a
/// local deploy; item ownership without the escrow lease overlay, which only
/// knows the leases active now.
#[async_trait]
impl BlockchainChecker for Synced<'_> {
    async fn check_land_access(
        &self,
        eth_address: &str,
        parcels: &[(i32, i32)],
        timestamp: Timestamp,
    ) -> Result<Vec<bool>, ValidatorError> {
        self.sources
            .check_land_access(eth_address, parcels, timestamp)
            .await
    }

    async fn check_names_ownership(
        &self,
        eth_address: &str,
        names: &[String],
        timestamp: Timestamp,
    ) -> Result<PermissionResult, ValidatorError> {
        self.sources
            .check_names_ownership(eth_address, names, timestamp)
            .await
    }

    async fn check_items_ownership(
        &self,
        eth_address: &str,
        urns: &[String],
        timestamp: Timestamp,
    ) -> Result<PermissionResult, ValidatorError> {
        let owned = self
            .sources
            .access()
            .items_capped(
                eth_address,
                urns,
                timestamp,
                super::onchain::SYNCED_ITEM_CAPS,
            )
            .await?;
        // The reference skips URNs it cannot parse, so they count as owned.
        Ok(PermissionResult::from_failing(
            urns.iter()
                .zip(owned)
                .filter(|(u, o)| !o && !super::onchain::is_malformed_item(u))
                .map(|(u, _)| u.clone())
                .collect(),
        ))
    }

    async fn check_collection_access(
        &self,
        eth_address: &str,
        contract_address: &str,
        item_id: &str,
        deployment: &Deployment<'_>,
        timestamp: Timestamp,
        layer: BlockchainLayer,
    ) -> Result<bool, ValidatorError> {
        self.sources
            .check_collection_access(
                eth_address,
                contract_address,
                item_id,
                deployment,
                timestamp,
                layer,
            )
            .await
    }

    async fn check_third_party_access(
        &self,
        asset_urn: &str,
        entity: &Entity,
        deployment: &Deployment<'_>,
        timestamp: Timestamp,
    ) -> Result<bool, ValidatorError> {
        self.sources
            .check_third_party_access(asset_urn, entity, deployment, timestamp)
            .await
    }

    fn is_address_owned_by_decentraland(&self, address: &str) -> bool {
        self.sources.is_address_owned_by_decentraland(address)
    }
}

/// How far ahead of this node's clock a synced entity may be and still wait
/// for it; beyond it the entity is refused. A node accepts a deploy at most
/// `REQUEST_TTL_FORWARDS_MS` ahead of its own clock, so only a skew of a day
/// between two nodes' clocks would put an honest entity past it.
const MAX_FUTURE_MS: i64 = 24 * 60 * 60 * 1000;

/// The longest one synced entity's validation (signature and access) takes
/// before it is undecided; the access step's deadline too.
pub(crate) const SYNCED_DEADLINE: Duration = Duration::from_secs(60);

/// Validates the entity file `entity_bytes` (already proven to hash to
/// `entity_id`) under `auth_chain`. `Ok` carries the expirations kept on the
/// peer's word; `Err` the errors with their kind, as [`super::validate`].
pub async fn validate_synced(
    sources: &Sources,
    entity_bytes: &[u8],
    entity_id: &str,
    auth_chain: &AuthChain,
    content: SyncedContent,
    access_passed: bool,
) -> Result<ChainVerdict, Vec<ValidationError>> {
    validate_synced_within(
        SYNCED_DEADLINE,
        sources,
        entity_bytes,
        entity_id,
        auth_chain,
        content,
        access_passed,
    )
    .await
}

async fn validate_synced_within(
    deadline: Duration,
    sources: &Sources,
    entity_bytes: &[u8],
    entity_id: &str,
    auth_chain: &AuthChain,
    content: SyncedContent,
    access_passed: bool,
) -> Result<ChainVerdict, Vec<ValidationError>> {
    let entity = entity_parser::parse_entity_from_bytes(entity_bytes, entity_id)
        .map_err(|e| vec![ValidationError::Rejected(e.to_string())])?;
    // Never judged ahead of this node's clock: retried until it catches up,
    // or refused past any honest clock skew.
    let now = chrono::Utc::now().timestamp_millis();
    if entity.timestamp > now + MAX_FUTURE_MS {
        return Err(vec![ValidationError::Rejected(format!(
            "The entity timestamp {} is more than a day in the future",
            entity.timestamp
        ))]);
    }
    if entity.timestamp > now + crate::write_deployer::REQUEST_TTL_FORWARDS_MS {
        return Err(vec![ValidationError::Unavailable(format!(
            "The entity timestamp {} is too far in the future",
            entity.timestamp
        ))]);
    }
    let files = HashMap::from([(
        entity_id.to_string(),
        bytes::Bytes::copy_from_slice(entity_bytes),
    )]);
    let synced = Synced {
        sources,
        content,
        trusted_expirations: Mutex::new(Vec::new()),
    };
    let validation = ContentValidator {
        external_calls: &synced,
        blockchain_checker: &synced,
        ignore_blockchain_access: sources.ignore_blockchain_access,
        synced: true,
        access_passed,
    };
    let deployment = Deployment {
        entity: &entity,
        files: &files,
        auth_chain,
    };
    let checks = validation.validate(&deployment);
    tokio::time::timeout(deadline, checks)
        .await
        .unwrap_or_else(|_| {
            ValidationResponse::unavailable(format!(
                "The validation did not finish within {} s",
                deadline.as_secs_f32()
            ))
        })
        .into_result()?;
    Ok(ChainVerdict {
        trusted_expirations: synced.trusted_expirations.into_inner().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::types::EntityType;
    use super::*;
    use crate::entity::ContentMapping;

    fn profile(content: &[(&str, &str)]) -> Entity {
        Entity {
            id: "bafyprofile".into(),
            entity_type: EntityType::Profile,
            pointers: vec!["0xabc".into()],
            timestamp: 1,
            content: content
                .iter()
                .map(|(file, hash)| ContentMapping {
                    file: file.to_string(),
                    hash: hash.to_string(),
                })
                .collect(),
            version: "v3".into(),
            metadata: None,
        }
    }

    const HAT_BY_NAME: &str = "urn:decentraland:ethereum:collections-v1:halloween_2019:hat";
    const HAT_BY_ADDRESS: &str =
        "urn:decentraland:ethereum:collections-v1:0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2:hat";
    const MASK_BY_NAME: &str = "urn:decentraland:ethereum:collections-v1:halloween_2019:mask";

    const MODEL: &[u8] = b"glTF";

    /// A wearable file on `pointers` whose thumbnail is not among its content.
    fn wearable_file(pointers: &[&str], timestamp: i64) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "version": "v3", "type": "wearable", "pointers": pointers,
            "timestamp": timestamp,
            "content": [{"file": "male/hat.glb", "hash": crate::auth::hash_bytes_v1(MODEL)}],
            "metadata": {
                "id": pointers[0], "name": "hat", "description": "",
                "i18n": [{"code": "en", "text": "Hat"}],
                "thumbnail": "thumbnail.png", "image": "image.png",
                "collectionAddress": "0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2",
                "rarity": "epic",
                "data": {
                    "replaces": [], "hides": [], "tags": ["hat"], "category": "hat",
                    "representations": [{
                        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
                        "mainFile": "male/hat.glb", "contents": ["male/hat.glb"],
                        "overrideHides": [], "overrideReplaces": []
                    }]
                }
            }
        }))
        .unwrap()
    }

    /// A signed wearable with no thumbnail on `pointers`, and the sources over
    /// a temp store holding its model.
    async fn wearable(
        pointers: &[&str],
    ) -> (Vec<u8>, String, AuthChain, Sources, std::path::PathBuf) {
        let bytes = wearable_file(pointers, 1_700_000_000_000);
        let id = crate::auth::hash_bytes_v1(&bytes);
        let chain = crate::sync_test_support::signed_chain(&id).await;
        let (storage, tmp) = crate::sync_test_support::temp_storage("synced-item").await;
        storage
            .store(&crate::auth::hash_bytes_v1(MODEL), MODEL.into())
            .await
            .unwrap();
        let sources = Sources::new(
            sqlx::PgPool::connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap(),
            storage,
            crate::http::Outbound::for_tests(),
            None,
            // collections-v1 items are deployed by the Decentraland address.
            super::super::Settings {
                additional_dcl_address: Some(
                    "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".to_string(),
                ),
                ..Default::default()
            },
        );
        (bytes, id, chain, sources, tmp)
    }

    #[tokio::test]
    async fn a_synced_item_skips_the_thumbnail_check_a_local_deploy_keeps() {
        let (bytes, id, chain, sources, tmp) = wearable(&[HAT_BY_NAME]).await;
        validate_synced(&sources, &bytes, &id, &chain, SyncedContent::Fetched, false)
            .await
            .expect("no thumbnail check on a synced item");

        let entity = entity_parser::parse_entity_from_bytes(&bytes, &id).unwrap();
        let files = HashMap::from([(id.clone(), bytes::Bytes::from(bytes))]);
        let local = super::super::validate(
            &entity,
            &files,
            &super::super::Context {
                sources: &sources,
                auth_chain: &chain,
            },
        )
        .await
        .unwrap_err();
        assert!(
            matches!(&local[..], [ValidationError::Rejected(m)] if m.starts_with("Couldn't find hash for thumbnail")),
            "{local:?}"
        );
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn a_synced_item_naming_one_asset_two_ways_has_one_pointer() {
        let (bytes, id, chain, sources, tmp) = wearable(&[HAT_BY_NAME, HAT_BY_ADDRESS]).await;
        validate_synced(&sources, &bytes, &id, &chain, SyncedContent::Fetched, false)
            .await
            .expect("both pointers resolve to the same asset");

        let (bytes, id, chain, sources, tmp2) = wearable(&[HAT_BY_NAME, MASK_BY_NAME]).await;
        let errors = validate_synced(&sources, &bytes, &id, &chain, SyncedContent::Fetched, false)
            .await
            .unwrap_err();
        assert!(
            matches!(&errors[..], [ValidationError::Rejected(m)] if m.starts_with("Only one pointer is allowed")),
            "{errors:?}"
        );
        let _ = std::fs::remove_dir_all(tmp);
        let _ = std::fs::remove_dir_all(tmp2);
    }

    /// Records the block each historical ask names.
    #[derive(Default)]
    struct AsksAt(std::sync::Mutex<Vec<u64>>);

    #[async_trait]
    impl Eip1654Validator for AsksAt {
        async fn validate_signature(&self, _: &str, _: &[u8], _: &[u8]) -> Result<bool, AuthError> {
            panic!("a synced link is never asked at latest")
        }

        async fn validate_signature_at_block(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
            block: u64,
        ) -> Result<bool, AuthError> {
            self.0.lock().unwrap().push(block);
            Ok(true)
        }
    }

    #[tokio::test]
    async fn a_contract_wallet_is_asked_at_the_block_the_access_checks_found() {
        use super::super::fake_chain::{self, GENESIS, INTERVAL};
        let chain = fake_chain::serve(|q, _| panic!("unexpected question {q:?}")).await;
        let (storage, tmp) = crate::sync_test_support::temp_storage("synced-block").await;
        let sources = Sources::new(
            sqlx::PgPool::connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap(),
            storage,
            crate::http::Outbound::for_tests(),
            None,
            super::super::Settings {
                eth_network: "mainnet".into(),
                l1_rpc_url: Some(chain.url.clone()),
                ..Default::default()
            },
        );
        let asks = AsksAt::default();
        let at = |timestamp_ms| AtEntityBlock {
            inner: &asks,
            sources: &sources,
            timestamp_ms,
        };
        let timestamp_ms = 1_700_000_000_500;

        sources.access.l1_block_at(timestamp_ms).await.unwrap();
        chain.clear();
        assert!(
            at(timestamp_ms)
                .validate_signature("0xabc", &[0; 32], &[0; 65])
                .await
                .unwrap()
        );
        assert_eq!(
            *asks.0.lock().unwrap(),
            [((1_700_000_000 - GENESIS) / INTERVAL) as u64]
        );
        assert!(
            chain.calls().is_empty(),
            "the block search is shared: {:?}",
            chain.calls()
        );

        let before_genesis = (GENESIS - 1) * 1000;
        assert!(
            !at(before_genesis)
                .validate_signature("0xabc", &[0; 32], &[0; 65])
                .await
                .unwrap(),
            "no block yet: a refusal"
        );
        assert_eq!(asks.0.lock().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(tmp);
    }

    /// A contract wallet whose RPC never answers.
    struct Stalled;

    #[async_trait]
    impl Eip1654Validator for Stalled {
        async fn validate_signature(&self, _: &str, _: &[u8], _: &[u8]) -> Result<bool, AuthError> {
            std::future::pending().await
        }

        async fn validate_signature_at_block(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
            _: u64,
        ) -> Result<bool, AuthError> {
            std::future::pending().await
        }
    }

    #[tokio::test]
    async fn a_synced_validation_past_its_deadline_is_undecided() {
        use super::super::fake_chain;
        const CONTRACT: &str = "0x0000000000000000000000000000000000000002";
        let chain = fake_chain::serve(|q, _| panic!("unexpected question {q:?}")).await;
        let (storage, tmp) = crate::sync_test_support::temp_storage("synced-deadline").await;
        let sources = Sources::new(
            sqlx::PgPool::connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap(),
            storage,
            crate::http::Outbound::for_tests(),
            Some(std::sync::Arc::new(Stalled)),
            super::super::Settings {
                eth_network: "mainnet".into(),
                l1_rpc_url: Some(chain.url.clone()),
                ..Default::default()
            },
        );
        let bytes = serde_json::to_vec(&serde_json::json!({
            "version": "v3", "type": "outfits", "pointers": [format!("{CONTRACT}:outfits")],
            "timestamp": 1_700_000_000_000i64, "content": [],
            "metadata": {"outfits": [], "namesForExtraSlots": []}
        }))
        .unwrap();
        let id = crate::auth::hash_bytes_v1(&bytes);
        let auth =
            crate::sync_test_support::signed_chain_behind_contract(&id, "2099-01-01T00:00:00.000Z")
                .await;

        let validation = validate_synced_within(
            Duration::from_millis(200),
            &sources,
            &bytes,
            &id,
            &auth,
            SyncedContent::Fetched,
            false,
        );
        let errors = tokio::time::timeout(Duration::from_secs(10), validation)
            .await
            .expect("bounded by the entity deadline")
            .unwrap_err();
        assert!(
            matches!(&errors[..], [ValidationError::Unavailable(m)] if m.contains("did not finish")),
            "{errors:?}"
        );
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn an_entity_from_the_future_waits_for_the_clock_and_past_a_day_is_refused() {
        let (_, _, _, sources, tmp) = wearable(&[HAT_BY_NAME]).await;
        let at = |timestamp: i64| {
            let bytes = wearable_file(&[HAT_BY_NAME], timestamp);
            let id = crate::auth::hash_bytes_v1(&bytes);
            (bytes, id)
        };
        let now = chrono::Utc::now().timestamp_millis();

        let (bytes, id) = at(now + 2 * crate::write_deployer::REQUEST_TTL_FORWARDS_MS);
        let chain = crate::sync_test_support::signed_chain(&id).await;
        let errors = validate_synced(&sources, &bytes, &id, &chain, SyncedContent::Fetched, false)
            .await
            .unwrap_err();
        assert!(
            matches!(&errors[..], [ValidationError::Unavailable(m)] if m.contains("too far in the future")),
            "{errors:?}"
        );

        let (bytes, id) = at(now + 60_000);
        let chain = crate::sync_test_support::signed_chain(&id).await;
        validate_synced(&sources, &bytes, &id, &chain, SyncedContent::Fetched, false)
            .await
            .expect("inside the clock skew");

        let (bytes, id) = at(now + MAX_FUTURE_MS + 60_000);
        let chain = crate::sync_test_support::signed_chain(&id).await;
        let errors = validate_synced(&sources, &bytes, &id, &chain, SyncedContent::Fetched, false)
            .await
            .unwrap_err();
        assert!(
            matches!(&errors[..], [ValidationError::Rejected(m)] if m.contains("more than a day")),
            "past a day ahead it is refused: {errors:?}"
        );
        let _ = std::fs::remove_dir_all(tmp);
    }

    /// Synced scenes validated per second over a chain whose every RPC answer takes `latency`.
    /// `SYNCED_BENCH_LATENCY_MS`, `SYNCED_BENCH_CONCURRENCY`, `SYNCED_BENCH_N`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn bench_synced_scene_throughput() {
        use super::super::fake_chain::{self, Query, Reply};
        use futures::StreamExt;
        let env = |k: &str, d: u64| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(d)
        };
        let latency = std::time::Duration::from_millis(env("SYNCED_BENCH_LATENCY_MS", 20));
        let concurrency = env("SYNCED_BENCH_CONCURRENCY", 50) as usize;
        let n = env("SYNCED_BENCH_N", 300) as usize;
        let passes = env("SYNCED_BENCH_PASSES", 1);

        let chain = fake_chain::serve(|q, _| match q {
            Query::Land { .. } => Reply::Yes(true),
            other => panic!("unexpected question {other:?}"),
        })
        .await;
        let upstream = chain.url.clone();
        let client = reqwest::Client::new();
        let proxy = axum::Router::new().route(
            "/",
            axum::routing::post(move |body: axum::body::Bytes| {
                let (client, upstream) = (client.clone(), upstream.clone());
                async move {
                    tokio::time::sleep(latency).await;
                    let resp = client
                        .post(&upstream)
                        .header("content-type", "application/json")
                        .body(body)
                        .send()
                        .await
                        .unwrap();
                    (
                        [("content-type", "application/json")],
                        resp.bytes().await.unwrap(),
                    )
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, proxy).await.unwrap() });

        let (storage, tmp) = crate::sync_test_support::temp_storage("bench").await;
        let sources = Sources::new(
            sqlx::PgPool::connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap(),
            storage,
            crate::http::Outbound::for_tests(),
            None,
            super::super::Settings {
                eth_network: "mainnet".into(),
                l1_rpc_url: Some(url.clone()),
                l2_rpc_url: Some(url),
                ..Default::default()
            },
        );
        let now = chrono::Utc::now().timestamp_millis();
        let mut entities = Vec::with_capacity(n);
        for i in 0..n {
            // Spread over the last year, as a bootstrap replays history.
            let timestamp = now - 86_400_000 - (i as i64 * 7_919_993_000) % 31_536_000_000;
            let parcel = format!("{},{}", i % 150, i / 150);
            let bytes = serde_json::to_vec(&serde_json::json!({
                "version": "v3", "type": "scene", "pointers": [parcel], "timestamp": timestamp,
                "content": [],
                "metadata": {"main": "bin/index.js", "scene": {"base": parcel, "parcels": [parcel]}}
            }))
            .unwrap();
            let id = crate::auth::hash_bytes_v1(&bytes);
            let chain = crate::sync_test_support::signed_chain(&id).await;
            entities.push((bytes, id, chain));
        }

        let started = std::time::Instant::now();
        futures::stream::iter(&entities)
            .for_each_concurrent(concurrency, |(bytes, id, auth)| {
                let sources = &sources;
                async move {
                    if passes > 1 {
                        validate_synced(sources, bytes, id, auth, SyncedContent::Pending, false)
                            .await
                            .unwrap();
                    }
                    validate_synced(sources, bytes, id, auth, SyncedContent::Fetched, passes > 1)
                        .await
                        .unwrap();
                }
            })
            .await;
        let elapsed = started.elapsed();
        let calls = chain.calls();
        let blocks = calls
            .iter()
            .filter(|c| c.starts_with("eth_getBlock"))
            .count();
        println!(
            "latency={latency:?} concurrency={concurrency} n={n} passes={passes}: {:.1} entities/s, \
             {:.2} eth_call and {:.2} block reads per entity",
            n as f64 / elapsed.as_secs_f64(),
            (calls.len() - blocks) as f64 / n as f64,
            blocks as f64 / n as f64,
        );
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn a_synced_profile_may_wear_a_collection_urn_naming_no_item_a_local_one_may_not() {
        const OWNER: &str = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
        let malformed = "urn:decentraland:matic:collections-v2:0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2:0:0x2a";
        let entity = Entity {
            metadata: Some(serde_json::json!({ "avatars": [{
                "name": "", "hasClaimedName": false,
                "avatar": { "wearables": [malformed], "emotes": [] }
            }]})),
            pointers: vec![OWNER.into()],
            timestamp: 1_700_000_000_000,
            ..profile(&[])
        };
        let files = HashMap::new();
        let chain = AuthChain::new();
        let deployment = Deployment {
            entity: &entity,
            files: &files,
            auth_chain: &chain,
        };
        // The local check reads the escrow overlay.
        let Some(pool) = crate::sync_test_support::test_pool(1).await else {
            return;
        };
        let (storage, tmp) = crate::sync_test_support::temp_storage("malformed-urn").await;
        let sources = Sources::new(
            pool,
            storage,
            crate::http::Outbound::for_tests(),
            None,
            super::super::Settings {
                eth_network: "mainnet".into(),
                ..Default::default()
            },
        );
        let synced = Synced {
            sources: &sources,
            content: SyncedContent::Fetched,
            trusted_expirations: Mutex::new(Vec::new()),
        };

        let verdict =
            super::super::checker::validate_profile_access(&deployment, &synced, OWNER).await;
        assert!(verdict.is_ok(), "{verdict:?}");
        let local =
            super::super::checker::validate_profile_access(&deployment, &sources, OWNER).await;
        let local = local.into_result().unwrap_err();
        assert!(
            matches!(&local[..], [ValidationError::Rejected(m)] if m.contains(malformed)),
            "a local deploy stays strict: {local:?}"
        );
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn a_synced_entity_is_not_refused_items_past_the_local_cap() {
        use super::super::fake_chain::{self, Query, Reply};
        const OWNER: &str = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
        let chain = fake_chain::serve(|q, _| match q {
            Query::OwnerOf { .. } => Reply::Owner(OWNER.parse().unwrap()),
            other => panic!("unexpected question {other:?}"),
        })
        .await;
        let (storage, tmp) = crate::sync_test_support::temp_storage("synced-caps").await;
        let sources = Sources::new(
            sqlx::PgPool::connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap(),
            storage,
            crate::http::Outbound::for_tests(),
            None,
            super::super::Settings {
                eth_network: "mainnet".into(),
                l2_rpc_url: Some(chain.url.clone()),
                ..Default::default()
            },
        );
        let synced = Synced {
            sources: &sources,
            content: SyncedContent::Fetched,
            trusted_expirations: Mutex::new(Vec::new()),
        };
        let urns: Vec<String> = (0..=super::super::onchain::MAX_ITEMS_ASKED)
            .map(|t| {
                format!(
                    "urn:decentraland:matic:collections-v2:0x0002000000000000000000000000000000000001:0:{t}"
                )
            })
            .collect();

        let verdict = synced
            .check_items_ownership(OWNER, &urns, 1_700_000_000_000)
            .await
            .unwrap();
        assert!(verdict.result, "{verdict:?}");
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn a_synced_profile_does_not_own_an_item_whose_token_id_overflows() {
        use super::super::fake_chain;
        const OWNER: &str = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
        let chain = fake_chain::serve(|q, _| panic!("unexpected question {q:?}")).await;
        let (storage, tmp) = crate::sync_test_support::temp_storage("synced-overflow").await;
        let sources = Sources::new(
            sqlx::PgPool::connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap(),
            storage,
            crate::http::Outbound::for_tests(),
            None,
            super::super::Settings {
                eth_network: "mainnet".into(),
                l1_rpc_url: Some(chain.url.clone()),
                l2_rpc_url: Some(chain.url.clone()),
                ..Default::default()
            },
        );
        let synced = Synced {
            sources: &sources,
            content: SyncedContent::Fetched,
            trusted_expirations: Mutex::new(Vec::new()),
        };
        let huge = "9".repeat(80);
        let contract = "0x0002000000000000000000000000000000000001";
        let urns = vec![
            format!("urn:decentraland:matic:collections-v2:{contract}:0:{huge}"),
            format!("urn:decentraland:matic:collections-thirdparty:tp:c:i:matic:{contract}:{huge}"),
        ];

        let verdict = synced
            .check_items_ownership(OWNER, &urns, 1_700_000_000_000)
            .await
            .unwrap();
        assert!(!verdict.result, "{verdict:?}");
        assert_eq!(verdict.failing, Some(urns));
        let _ = std::fs::remove_dir_all(tmp);
    }
}
