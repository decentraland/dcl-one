//! The validator over an entity a peer serves (sync, the failed-deployments
//! retries, the read-through): every rule [`super::validate`] runs, minus the
//! server-side checks the old node skipped for synced deployments (request
//! recency, rate limits, newer entity on the pointers, which live in the
//! write deployer and never reach the validator).
//!
//! What differs from a local deployment:
//! - the files are the entity file alone; content files are read from the
//!   store sync downloaded them into. Profile render files sync deliberately
//!   does not download (`PROFILE_RENDER_FILES`) count as present with size 0.
//!   [`SyncedContent::Pending`] (the read-through's check before it stores
//!   anything) counts every content file as present.
//! - the auth chain: expiry against the entity timestamp, an expiration that
//!   parses as no known date kept as the peer's verdict, a contract-wallet
//!   link asked of the node's EIP-1654 validator (none configured:
//!   undecidable).
//! - access: every check asks the chain at the entity timestamp's block, then
//!   five minutes before it ([`super::onchain`]), as for a local deploy; item
//!   ownership leaves out the escrow lease overlay, which knows only the
//!   leases active now. A source unset or failing, or a node without that
//!   block's state, is undecidable; a revert is a "no".

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use async_trait::async_trait;

use super::checker::{BlockchainChecker, BlockchainLayer};
use super::error::{PermissionResult, ValidationError, ValidatorError};
use super::rules::{ContentValidator, ExternalCalls};
use super::types::{Deployment, Entity, EntityType, Timestamp};
use super::{Sources, entity_parser};
use crate::auth::verify::{ChainVerdict, verify_synced_auth_chain};
use crate::auth::{AuthChain, AuthError};

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
    /// Hashes the sync path does not download; present with size 0.
    not_fetched: HashSet<String>,
    content: SyncedContent,
    trusted_expirations: Mutex<Vec<String>>,
}

impl Synced<'_> {
    fn assumed_present(&self, hash: &str) -> bool {
        self.content == SyncedContent::Pending || self.not_fetched.contains(hash)
    }
}

#[async_trait]
impl ExternalCalls for Synced<'_> {
    async fn is_content_stored_already(
        &self,
        hashes: &[String],
    ) -> Result<HashMap<String, bool>, String> {
        let ask: Vec<String> = hashes
            .iter()
            .filter(|h| !self.assumed_present(h))
            .cloned()
            .collect();
        let mut stored = if ask.is_empty() {
            HashMap::new()
        } else {
            self.sources.is_content_stored_already(&ask).await?
        };
        for hash in hashes {
            if self.assumed_present(hash) {
                stored.insert(hash.clone(), true);
            }
        }
        Ok(stored)
    }

    async fn fetch_content_file_size(&self, hash: &str) -> Result<Option<usize>, String> {
        if self.assumed_present(hash) {
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
        let verdict =
            verify_synced_auth_chain(auth_chain, entity_id, timestamp, self.sources.eip1654())
                .await?;
        if let Ok(mut trusted) = self.trusted_expirations.lock() {
            trusted.extend(verdict.trusted_expirations);
        }
        Ok(())
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
            .items(eth_address, urns, timestamp)
            .await?;
        Ok(PermissionResult::from_failing(
            urns.iter()
                .zip(owned)
                .filter(|(_, o)| !o)
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

/// The content hashes of `entity` that sync leaves undownloaded: profile
/// render files no other file of the entity shares.
fn not_fetched_hashes(entity: &Entity) -> HashSet<String> {
    if entity.entity_type != EntityType::Profile {
        return HashSet::new();
    }
    let (renders, others): (Vec<_>, Vec<_>) = entity
        .content
        .iter()
        .partition(|c| crate::deploy_remote_entity::is_profile_render_file(&c.file));
    let fetched: HashSet<&str> = others.iter().map(|c| c.hash.as_str()).collect();
    renders
        .into_iter()
        .filter(|c| !fetched.contains(c.hash.as_str()))
        .map(|c| c.hash.clone())
        .collect()
}

/// Validates the entity file `entity_bytes` (already proven to hash to
/// `entity_id`) under `auth_chain`. `Ok` carries the expirations kept on the
/// peer's word; `Err` the errors with their kind, as [`super::validate`].
pub async fn validate_synced(
    sources: &Sources,
    entity_bytes: &[u8],
    entity_id: &str,
    auth_chain: &AuthChain,
    content: SyncedContent,
) -> Result<ChainVerdict, Vec<ValidationError>> {
    let entity = entity_parser::parse_entity_from_bytes(entity_bytes, entity_id)
        .map_err(|e| vec![ValidationError::Rejected(e.to_string())])?;
    let files = HashMap::from([(entity_id.to_string(), entity_bytes.to_vec())]);
    let synced = Synced {
        sources,
        not_fetched: not_fetched_hashes(&entity),
        content,
        trusted_expirations: Mutex::new(Vec::new()),
    };
    ContentValidator {
        external_calls: &synced,
        blockchain_checker: &synced,
        ignore_blockchain_access: sources.ignore_blockchain_access,
    }
    .validate(&Deployment {
        entity: &entity,
        files: &files,
        auth_chain,
    })
    .await
    .into_result()?;
    Ok(ChainVerdict {
        trusted_expirations: synced.trusted_expirations.into_inner().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn only_profile_renders_no_other_file_shares_are_left_unfetched() {
        let entity = profile(&[
            ("face256.png", "bafyface"),
            ("./body.png", "bafybody"),
            ("face.png", "bafyshared"),
            ("avatar.glb", "bafyshared"),
            ("model.glb", "bafymodel"),
        ]);
        assert_eq!(
            not_fetched_hashes(&entity),
            HashSet::from(["bafyface".to_string(), "bafybody".to_string()])
        );

        let mut scene = entity;
        scene.entity_type = EntityType::Scene;
        assert!(not_fetched_hashes(&scene).is_empty());
    }
}
