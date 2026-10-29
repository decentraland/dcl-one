//! The content validator of the local write path: the port of
//! `catalyrst-validator` (itself the Rust port of `@dcl/content-validator`).
//! [`validate`] runs the old `ContentValidator` over one deployment in the
//! old order ([`rules`]): entity structure, IPFS hashing and ADR-45, the auth
//! chain, the size cap of the entity type (skins 9 MB), wearable and emote
//! representations and spring bones, the profile rules (ADR-74/75/232/244/290,
//! emote slots), the item and face256 thumbnails ([`image_metadata`]), the
//! scene rules (parcels match pointers, ADR-173, ADR-236), the content
//! cross-check, the outfits rules, the third-party merkle proof content
//! ([`third_party`]) and the access step per type ([`checker`]), and answers
//! with the old error strings byte for byte.
//!
//! Where the old checker read the squid, [`Sources`] reads what replaced it
//! (`chain.rs` has the table): LAND rights through
//! `land_authz::can_deploy_batch` (the index when ready, else the
//! marketplace subgraph's owner leg), names and items and collection access
//! from the marketplace readers (plus the `shop.usage_grants` escrow
//! overlay), third-party roots from the registry subgraph at the deploy's L2
//! block ([`tp_subgraph`]), every upstream call through the ONE
//! `http::Outbound`, the overlay over the ONE pool, content probes against
//! the ONE blob store.
//!
//! Each error has a kind ([`ValidationError`]): `Rejected` is the
//! deployment's fault (400); `Unavailable` means this node could not decide
//! (its store could not be read, a source is unset or failed, a
//! contract-wallet signature could not be asked about) and the write path
//! answers 503 with `Retry-After`. The old validator rejected on a checker
//! failure ("Error checking ...: ..."); the same text is now `Unavailable`.
//!
//! Not carried: `squid_checker.rs` (replaced as above, with its
//! `LandOperatorResolver`, which `land_authz` owns), the squid root source of
//! third-party wearables and its refresh job, `land_rights.rs` (the types
//! live in `land_authz::resolve`; their tests are in `chain.rs`),
//! `ExternalCalls::calculate_files_hashes` and
//! `BlockchainChecker::{find_blocks_for_timestamp, check_third_party_items}`
//! (no caller), the ts-rs bindings.

mod chain;
pub mod checker;
pub mod entity_parser;
pub mod erc721;
pub mod error;
mod helpers;
pub mod image_metadata;
pub mod merkle;
pub mod rules;
mod scene_parcels;
mod spring_bones;
pub mod third_party;
pub mod tp_subgraph;
pub mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::bail;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::auth::{AuthChain, AuthError, Eip1654Validator};
use crate::chain_index::ChainIndex;
use crate::http::Outbound;
use crate::land_authz::LandAuthzState;
use crate::storage::{Storage, StorageError};

pub use chain::DECENTRALAND_ADDRESS;
pub use error::{ValidationError, ValidationResponse, ValidatorError};
pub use rules::{ContentValidator, ExternalCalls};
pub use spring_bones::validate_spring_bones_metadata;
pub use types::{Deployment, Entity, EntityType};

/// The validator's configuration, under the old environment names.
#[derive(Clone, Default)]
pub struct Settings {
    /// `IGNORE_BLOCKCHAIN_ACCESS_CHECKS`: the access step runs only the
    /// local pointer-ownership rule (profiles, outfits, stores); for a node
    /// that must accept deploys without its chain sources. Off by default.
    pub ignore_blockchain_access: bool,
    /// `ADDITIONAL_DECENTRALAND_ADDRESS`: a second address with the
    /// Decentraland address's rights (default profiles, off-chain items,
    /// legacy content, v1 collection items).
    pub additional_dcl_address: Option<String>,
    /// `THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL`.
    pub third_party_registry_url: Option<String>,
    /// `BLOCKS_L2_SUBGRAPH_URL`.
    pub blocks_l2_url: Option<String>,
}

/// No derived `Debug`: a subgraph URL can carry a gateway key.
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("ignore_blockchain_access", &self.ignore_blockchain_access)
            .field("additional_dcl_address", &self.additional_dcl_address)
            .field(
                "third_party_registry_url",
                &self.third_party_registry_url.is_some(),
            )
            .field("blocks_l2_url", &self.blocks_l2_url.is_some())
            .finish()
    }
}

impl Settings {
    /// The values as the environment spells them. The two third-party
    /// subgraphs come together or not at all, each a public https endpoint
    /// that is not this node (the old reader refused plaintext at startup
    /// too); an error names the variable, never the value.
    pub fn parse(
        ignore_blockchain_access: bool,
        additional_dcl_address: Option<&str>,
        third_party_registry_url: Option<&str>,
        blocks_l2_url: Option<&str>,
        self_hosts: &[String],
    ) -> anyhow::Result<Self> {
        let clean = |v: Option<&str>| {
            v.map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };
        let third_party_registry_url = clean(third_party_registry_url);
        let blocks_l2_url = clean(blocks_l2_url);
        for (var, url) in [
            (
                "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL",
                &third_party_registry_url,
            ),
            ("BLOCKS_L2_SUBGRAPH_URL", &blocks_l2_url),
        ] {
            if let Some(url) = url {
                crate::shared::upstream::check_upstream(var, url, self_hosts)?;
                if !url.starts_with("https://") {
                    bail!(
                        "{var} is not https: subgraph answers gate third-party deploys and are refused over plaintext"
                    );
                }
            }
        }
        if third_party_registry_url.is_some() != blocks_l2_url.is_some() {
            bail!(
                "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL and BLOCKS_L2_SUBGRAPH_URL are set together or not at all: \
                 a third-party root is read at the block of the deploy"
            );
        }
        let additional_dcl_address = clean(additional_dcl_address).map(|a| a.to_lowercase());
        if additional_dcl_address
            .as_deref()
            .is_some_and(|a| !crate::auth::is_eth_address(a))
        {
            bail!("ADDITIONAL_DECENTRALAND_ADDRESS is not an Ethereum address");
        }
        Ok(Self {
            ignore_blockchain_access,
            additional_dcl_address,
            third_party_registry_url,
            blocks_l2_url,
        })
    }
}

/// What the validator reads, built once by `serve`: the ONE pool (the
/// `shop.usage_grants` overlay), the ONE blob store, the shared EIP-1654
/// validator (`None` without `ETH_RPC_URL`: a contract-wallet link is then
/// undecidable), the land_authz state and chain index the rest of the node
/// uses, and the third-party subgraphs over the ONE client.
pub struct Sources {
    pool: PgPool,
    storage: Arc<Storage>,
    eip1654: Option<Arc<dyn Eip1654Validator>>,
    land: Option<Arc<LandAuthzState>>,
    market: Option<Arc<ChainIndex>>,
    third_party: Option<tp_subgraph::TpSubgraph>,
    ignore_blockchain_access: bool,
    additional_dcl_address: Option<String>,
}

impl Sources {
    pub fn new(
        pool: PgPool,
        storage: Arc<Storage>,
        http: Outbound,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
        land: Option<Arc<LandAuthzState>>,
        market: Option<Arc<ChainIndex>>,
        settings: Settings,
    ) -> Self {
        let third_party = match (settings.blocks_l2_url, settings.third_party_registry_url) {
            (Some(blocks), Some(registry)) => {
                Some(tp_subgraph::TpSubgraph::new(http, blocks, registry))
            }
            _ => None,
        };
        Self {
            pool,
            storage,
            eip1654,
            land,
            market,
            third_party,
            ignore_blockchain_access: settings.ignore_blockchain_access,
            additional_dcl_address: settings.additional_dcl_address,
        }
    }
}

#[async_trait]
impl ExternalCalls for Sources {
    async fn is_content_stored_already(
        &self,
        hashes: &[String],
    ) -> Result<HashMap<String, bool>, String> {
        self.storage
            .exist_multiple(hashes)
            .await
            .map_err(|e| e.to_string())
    }

    async fn fetch_content_file_size(&self, hash: &str) -> Result<Option<usize>, String> {
        match self.storage.file_info(hash).await {
            Ok(info) => Ok(info.map(|i| i.size as usize)),
            Err(StorageError::InvalidId(_) | StorageError::PathTraversal(_)) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// The ephemeral links are checked against the entity timestamp, as
    /// upstream does: a key that expired after signing still signed a valid
    /// deployment.
    async fn validate_signature(
        &self,
        entity_id: &str,
        auth_chain: &AuthChain,
        timestamp: types::Timestamp,
    ) -> Result<(), AuthError> {
        crate::auth::verify_auth_chain_async(
            auth_chain,
            entity_id,
            Some(timestamp),
            self.eip1654.as_deref(),
        )
        .await
    }
}

/// One deployment's validation: the signer's chain and the node's sources.
pub struct Context<'a> {
    pub sources: &'a Sources,
    pub auth_chain: &'a AuthChain,
}

/// Validates a local deployment: `entity` as parsed from its file (id set),
/// `files` the uploaded bytes by content hash with the entity file under
/// the entity id. `Err` carries the old error strings, every one of the same
/// kind.
pub async fn validate(
    entity: &Entity,
    files: &HashMap<String, Vec<u8>>,
    ctx: &Context<'_>,
) -> Result<(), Vec<ValidationError>> {
    let deployment = Deployment {
        entity,
        files,
        auth_chain: ctx.auth_chain,
    };
    ContentValidator {
        external_calls: ctx.sources,
        blockchain_checker: ctx.sources,
        ignore_blockchain_access: ctx.sources.ignore_blockchain_access,
    }
    .validate(&deployment)
    .await
    .into_result()
}

/// The write deployer's failure for a refused validation: `Unavailable`
/// (503) when this node could not decide, `Rejected` (400) otherwise.
pub fn into_deploy_failure(errors: Vec<ValidationError>) -> super::write_deployer::DeployFailure {
    use super::write_deployer::DeployFailure;
    let unavailable = errors.iter().any(ValidationError::is_unavailable);
    let messages: Vec<String> = errors
        .into_iter()
        .map(|e| match e {
            ValidationError::Rejected(m) | ValidationError::Unavailable(m) => m,
        })
        .collect();
    if unavailable {
        DeployFailure::Unavailable(messages)
    } else {
        DeployFailure::Rejected(messages)
    }
}
