//! The rules, in the old order: structure, IPFS hashing, metadata schema, ADR-45, signature,
//! size, item representations, item thumbnails, profile (upstream
//! `createProfileValidateFn` order: snapshots rejected, face thumbnail, images,
//! content rules), scene content, the content cross-check, outfits, the
//! third-party merkle proof content, access. The first failing step answers; its strings are
//! the old ones byte for byte, except the reasons after the metadata schema
//! sentence (see [`super::metadata_schema`]).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use async_trait::async_trait;
use tracing::{debug, warn};

use super::checker::{
    BlockchainChecker, validate_item_access, validate_outfits_access, validate_profile_access,
    validate_scene_access, validate_store_access,
};
use super::error::ValidationResponse;
use super::helpers::{
    avatar_has_face_and_body, avatar_has_snapshots, corresponds_to_a_snapshot,
    navmap_thumbnail_error, profile_has_emotes, should_validate_face_thumbnail, slot_key,
    validate_profile_emote_urns, validate_profile_wearable_urns,
    validates_up_to_adr290_optionality,
};
use super::image_metadata::{check_face256_thumbnail_image, check_wearable_thumbnail_image};
use super::scene_parcels::validate_scene_parcels_match_pointers;
use super::spring_bones::validate_spring_bones_metadata;
use super::types::*;
use crate::auth::{AuthChain, AuthError};
use crate::entity::is_ipfs_v2;

/// The longest an entity's validation, from the signature step to the end of
/// its access checks, takes before the deploy is undecided, well under the
/// usual client and proxy timeouts.
const ACCESS_DEADLINE: Duration = Duration::from_secs(30);

/// What the rules read from this node: its blob store and the auth-chain
/// verifier. [`super::Sources`] is the live implementation; the store-fault
/// tests stub it.
#[async_trait]
pub trait ExternalCalls: Send + Sync {
    /// `Ok(false)` / `Ok(None)` mean provable absence only. A store this node could not read is an
    /// `Err`, never a miss.
    async fn is_content_stored_already(
        &self,
        hashes: &[String],
    ) -> Result<HashMap<String, bool>, String>;

    async fn fetch_content_file_size(&self, hash: &str) -> Result<Option<usize>, String>;

    async fn validate_signature(
        &self,
        entity_id: &str,
        auth_chain: &AuthChain,
        timestamp: Timestamp,
    ) -> Result<(), AuthError>;
}

/// The cause is logged, never answered: it names this node's paths and errors.
fn content_store_unavailable(cause: &str) -> ValidationResponse {
    tracing::error!(cause, "content store unreadable during deploy validation");
    ValidationResponse::unavailable(
        "This node could not read its own content store, so the deployment could not be \
         validated. Please retry.",
    )
}

/// An uploaded file this node spooled and could not read back: its own
/// disk failing, answered as the spool's 503. The cause is logged.
fn upload_unreadable(error: &std::io::Error) -> ValidationResponse {
    tracing::error!(%error, "a spooled upload could not be read during deploy validation");
    ValidationResponse::unavailable(crate::create_entity::SPOOL_UNAVAILABLE_MESSAGE)
}

/// A wearable's or emote's `metadata.thumbnail` file name and the content
/// hash it maps to.
fn item_thumbnail(entity: &Entity) -> (Option<&str>, Option<&str>) {
    let thumbnail = entity
        .metadata
        .as_ref()
        .and_then(|m| m.get("thumbnail"))
        .and_then(|t| t.as_str());
    let hash = entity
        .content
        .iter()
        .find(|c| Some(c.file.as_str()) == thumbnail)
        .map(|c| c.hash.as_str());
    (thumbnail, hash)
}

/// An avatar's `snapshots.face256` content hash.
fn face256_of(avatar: &serde_json::Value) -> Option<&str> {
    avatar
        .get("avatar")
        .and_then(|a| a.get("snapshots"))
        .and_then(|s| s.get("face256"))
        .and_then(|h| h.as_str())
}

/// The uploaded files besides the entity file whose bytes validating
/// `entity` may read: a wearable's or emote's thumbnail, a profile's face256
/// snapshots. Each is read whole, one at a time, so the most a validation
/// holds on top of the entity file is the largest of them.
pub fn inspected_hashes(entity: &Entity) -> Vec<&str> {
    match entity.entity_type {
        EntityType::Wearable | EntityType::Emote => item_thumbnail(entity).1.into_iter().collect(),
        EntityType::Profile => entity
            .metadata
            .as_ref()
            .and_then(|m| m.get("avatars"))
            .and_then(|a| a.as_array())
            .map(|avatars| avatars.iter().filter_map(face256_of).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub struct ContentValidator<'a, E: ExternalCalls, B: BlockchainChecker> {
    pub external_calls: &'a E,
    pub blockchain_checker: &'a B,
    /// `IGNORE_BLOCKCHAIN_ACCESS_CHECKS`: only the local pointer-ownership
    /// rule of the access step runs.
    pub ignore_blockchain_access: bool,
    /// An entity a peer serves: the item thumbnail is not checked and an
    /// item's equivalent pointers count as one, as the reference does.
    /// A new deploy also needs integer emote and outfit slots, stricter than
    /// the reference's number; a synced entity keeps the numeric reading.
    pub synced: bool,
    /// The access step already passed for this entity and chain (the
    /// read-through's check before its content was in): not asked again.
    pub access_passed: bool,
}

impl<E: ExternalCalls, B: BlockchainChecker> ContentValidator<'_, E, B> {
    /// From the top of [`Self::validate`] to the end of the access checks: a synced entity's
    /// whole validation budget, so it has one deadline.
    fn access_budget(&self) -> Duration {
        if self.synced {
            super::synced::SYNCED_DEADLINE
        } else {
            ACCESS_DEADLINE
        }
    }

    /// The access step alone, within its own deadline.
    pub async fn validate_access_only(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let deadline = tokio::time::Instant::now() + self.access_budget();
        self.validate_access(deployment, deadline).await
    }

    pub async fn validate(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let deadline = tokio::time::Instant::now() + self.access_budget();
        let result = self.validate_common_prefix(deployment).await;
        if !result.is_ok() {
            return result;
        }

        let result = self.validate_size(deployment).await;
        if !result.is_ok() {
            debug!("Validation failed at size");
            return result;
        }

        let result = self.validate_items(deployment);
        if !result.is_ok() {
            debug!("Validation failed at item validation");
            return result;
        }

        let result = self.validate_item_thumbnail(deployment).await;
        if !result.is_ok() {
            debug!("Validation failed at item thumbnail");
            return result;
        }

        let result = self.validate_profile(deployment).await;
        if !result.is_ok() {
            debug!("Validation failed at profile");
            return result;
        }

        let result = self.validate_scene_content(deployment);
        if !result.is_ok() {
            debug!("Validation failed at scene content");
            return result;
        }

        let result = self.validate_content(deployment).await;
        if !result.is_ok() {
            debug!("Validation failed at content cross-check");
            return result;
        }

        let result = self.validate_outfits_content(deployment);
        if !result.is_ok() {
            debug!("Validation failed at outfits content");
            return result;
        }

        let result = match deployment.entity.entity_type {
            EntityType::Wearable => super::third_party::validate_third_party_merkle_proof_content(
                deployment,
                super::third_party::WEARABLE_REQUIRED_HASHING_KEYS,
                "wearable",
            ),
            EntityType::Emote => super::third_party::validate_third_party_merkle_proof_content(
                deployment,
                super::third_party::EMOTE_REQUIRED_HASHING_KEYS,
                "emote",
            ),
            _ => ValidationResponse::Ok,
        };
        if !result.is_ok() {
            debug!("Validation failed at third-party merkle proof");
            return result;
        }

        let result = self.validate_access(deployment, deadline).await;
        if !result.is_ok() {
            debug!("Validation failed at access check");
            return result;
        }

        ValidationResponse::Ok
    }

    /// The content-independent subset upstream's `createStagingValidator`
    /// runs on a batch of a partial upload (`partial_deployments`): the
    /// structure, hashing, metadata schema, ADR-45, signature and scene rules
    /// and the uploaded-but-unreferenced check, then the access step unless
    /// `include_access` is off (a resume by the upload's creator). The size
    /// and content completeness wait for publication, which runs
    /// [`Self::validate`] in full.
    pub async fn validate_staging(
        &self,
        deployment: &Deployment<'_>,
        include_access: bool,
    ) -> ValidationResponse {
        let deadline = tokio::time::Instant::now() + self.access_budget();
        let result = self.validate_common_prefix(deployment).await;
        if !result.is_ok() {
            return result;
        }

        let result = self.validate_scene_content(deployment);
        if !result.is_ok() {
            return result;
        }
        let referenced: HashSet<&String> =
            deployment.entity.content.iter().map(|c| &c.hash).collect();
        let unreferenced: Vec<String> = deployment
            .files
            .keys()
            .filter(|hash| !referenced.contains(hash) && **hash != deployment.entity.id)
            .map(|hash| {
                format!("This hash was uploaded but is not referenced in the entity: {hash}")
            })
            .collect();
        if !unreferenced.is_empty() {
            return ValidationResponse::from_errors(unreferenced);
        }
        if include_access {
            return self.validate_access(deployment, deadline).await;
        }
        ValidationResponse::Ok
    }

    async fn validate_common_prefix(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let result = self.validate_entity_structure(deployment);
        if !result.is_ok() {
            debug!("Validation failed at entity structure");
            return result;
        }

        let result = validate_ipfs_hashing(deployment.entity);
        if !result.is_ok() {
            debug!("Validation failed at IPFS hashing");
            return result;
        }

        let result = super::metadata_schema::validate_metadata(deployment.entity);
        if !result.is_ok() {
            debug!("Validation failed at metadata schema");
            return result;
        }

        let result = self.validate_adr45(deployment);
        if !result.is_ok() {
            debug!("Validation failed at ADR-45 check");
            return result;
        }

        let result = self.validate_signature(deployment).await;
        if !result.is_ok() {
            debug!("Validation failed at signature");
            return result;
        }

        ValidationResponse::Ok
    }

    fn validate_entity_structure(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;

        if entity.pointers.is_empty() {
            return ValidationResponse::fail(
                "The entity needs to be pointed by one or more pointers.".to_string(),
            );
        }

        let unique: HashSet<&String> = entity.pointers.iter().collect();
        if unique.len() != entity.pointers.len() {
            return ValidationResponse::fail(
                "There are repeated pointers in your request.".to_string(),
            );
        }

        ValidationResponse::Ok
    }

    fn validate_adr45(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;

        if entity.version != "v3" && entity.timestamp > adr_timestamps::ADR_45 {
            return ValidationResponse::fail(
                "Only entities v3 are allowed after the ADR-45. \
                 Check http://adr.decentraland.org/adr/ADR-45 for more information"
                    .to_string(),
            );
        }

        ValidationResponse::Ok
    }

    async fn validate_signature(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;
        match self
            .external_calls
            .validate_signature(&entity.id, deployment.auth_chain, entity.timestamp)
            .await
        {
            Ok(()) => ValidationResponse::Ok,
            // A contract-wallet link this node could not ask about (no
            // ETH_RPC_URL, or the RPC failed) is undecided, not invalid.
            Err(e @ (AuthError::Eip1654ValidationFailed(_) | AuthError::Eip1654NotImplemented)) => {
                ValidationResponse::unverifiable(format!("The signature is invalid. {e}"))
            }
            Err(e) => ValidationResponse::fail(format!("The signature is invalid. {e}")),
        }
    }

    async fn validate_size(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;

        if entity.timestamp <= adr_timestamps::LEGACY_CONTENT_MIGRATION {
            return ValidationResponse::Ok;
        }

        let max_size_mb = match max_size_mb(entity.entity_type) {
            Some(size) => size,
            None => {
                return ValidationResponse::fail(format!(
                    "Type {} is not supported yet",
                    entity.entity_type
                ));
            }
        };

        let max_size_mb = if entity.entity_type == EntityType::Wearable {
            if let Some(category) = entity
                .metadata
                .as_ref()
                .and_then(|m| m.get("data"))
                .and_then(|d| d.get("category"))
                .and_then(|c| c.as_str())
            {
                if category == "skin" {
                    SKIN_MAX_SIZE_MB
                } else {
                    max_size_mb
                }
            } else {
                max_size_mb
            }
        } else {
            max_size_mb
        };

        let max_size_bytes = max_size_mb * 1024 * 1024;

        let total_size = if entity.timestamp > adr_timestamps::ADR_45 {
            match self.calculate_deployment_size(deployment).await {
                Ok(size) => size,
                Err(response) => return response,
            }
        } else {
            deployment.files.values().map(UploadedFile::len).sum()
        };

        // Compares total / pointers > max without truncating the quotient.
        let allowed_bytes = (entity.pointers.len().max(1) as u64).saturating_mul(max_size_bytes);
        if total_size > allowed_bytes {
            return ValidationResponse::fail(format!(
                "The deployment is too big. The maximum allowed size per pointer is \
                 {max_size_mb} MB for {}. You can upload up to {} bytes but you tried \
                 to upload {total_size}.",
                entity.entity_type, allowed_bytes
            ));
        }

        ValidationResponse::Ok
    }

    async fn calculate_deployment_size(
        &self,
        deployment: &Deployment<'_>,
    ) -> Result<u64, ValidationResponse> {
        let mut total: u64 = 0;
        let unique_hashes: HashSet<&String> =
            deployment.entity.content.iter().map(|c| &c.hash).collect();

        for hash in unique_hashes {
            if let Some(uploaded) = deployment.files.get(hash) {
                total += uploaded.len();
            } else {
                match self.external_calls.fetch_content_file_size(hash).await {
                    Ok(Some(size)) => total += size as u64,
                    Ok(None) => {
                        return Err(ValidationResponse::fail(format!(
                            "Couldn't fetch content file with hash: {hash}"
                        )));
                    }
                    Err(msg) => return Err(content_store_unavailable(&msg)),
                }
            }
        }

        Ok(total)
    }

    fn validate_items(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;

        match entity.entity_type {
            EntityType::Wearable => {
                let result = self.validate_wearable_representations(deployment);
                if !result.is_ok() {
                    return result;
                }
                validate_spring_bones_metadata(deployment.entity)
            }
            EntityType::Emote => self.validate_emote_representations(deployment),
            _ => ValidationResponse::Ok,
        }
    }

    fn validate_wearable_representations(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let representations = deployment
            .entity
            .metadata
            .as_ref()
            .and_then(|m| m.get("data"))
            .and_then(|d| d.get("representations"));
        validate_representation_contents(deployment.entity, representations, "wearable")
    }

    fn validate_emote_representations(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;

        if entity.timestamp < adr_timestamps::ADR_74 {
            return ValidationResponse::fail(format!(
                "The emote timestamp {} is before ADR 74. \
                 Emotes did not exist before ADR 74.",
                entity.timestamp
            ));
        }

        let data = entity
            .metadata
            .as_ref()
            .and_then(|m| m.get("emoteDataADR74"));
        let result = validate_representation_contents(
            entity,
            data.and_then(|d| d.get("representations")),
            "emote",
        );
        if !result.is_ok() {
            return result;
        }
        validate_social_emote(data)
    }

    async fn validate_item_thumbnail(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;

        if self.synced || !matches!(entity.entity_type, EntityType::Wearable | EntityType::Emote) {
            return ValidationResponse::Ok;
        }
        if entity.timestamp < adr_timestamps::ADR_45 {
            return ValidationResponse::Ok;
        }

        let (thumbnail, hash) = item_thumbnail(entity);
        let hash = match hash {
            Some(h) => h.to_string(),
            None => {
                return ValidationResponse::fail(format!(
                    "Couldn't find hash for thumbnail file with name: {}",
                    thumbnail.unwrap_or("undefined")
                ));
            }
        };

        match deployment.files.get(&hash) {
            Some(file) => match file.read().await {
                Ok(buffer) => {
                    ValidationResponse::from_errors(check_wearable_thumbnail_image(&buffer))
                }
                Err(e) => upload_unreadable(&e),
            },
            None => {
                match self
                    .external_calls
                    .is_content_stored_already(std::slice::from_ref(&hash))
                    .await
                {
                    Ok(stored) if stored.get(&hash).copied().unwrap_or(false) => {
                        ValidationResponse::Ok
                    }
                    Ok(_) => ValidationResponse::fail(format!(
                        "Couldn't find thumbnail file with hash: {hash}"
                    )),
                    Err(msg) => content_store_unavailable(&msg),
                }
            }
        }
    }

    /// Upstream `createProfileValidateFn`: its rules in upstream order, the
    /// first failing one answering.
    async fn validate_profile(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        if deployment.entity.entity_type != EntityType::Profile {
            return ValidationResponse::Ok;
        }
        let result = self.validate_profile_snapshots_rejected(deployment);
        if !result.is_ok() {
            return result;
        }
        let result = self.validate_face_thumbnail(deployment).await;
        if !result.is_ok() {
            return result;
        }
        let result = self.validate_profile_images(deployment);
        if !result.is_ok() {
            return result;
        }
        self.validate_profile_content(deployment)
    }

    /// Upstream `profileMustNotHaveSnapshotsValidateFn`.
    fn validate_profile_snapshots_rejected(
        &self,
        deployment: &Deployment<'_>,
    ) -> ValidationResponse {
        let entity = &deployment.entity;
        if entity.timestamp < adr_timestamps::ADR_290_REJECTED {
            return ValidationResponse::Ok;
        }
        let avatars = entity
            .metadata
            .as_ref()
            .and_then(|m| m.get("avatars"))
            .and_then(|a| a.as_array())
            .map_or(&[][..], Vec::as_slice);
        if avatars.iter().any(avatar_has_snapshots) {
            return ValidationResponse::fail("Avatars must not have snapshots.".to_string());
        }
        ValidationResponse::Ok
    }

    async fn validate_face_thumbnail(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;
        if entity.entity_type != EntityType::Profile {
            return ValidationResponse::Ok;
        }
        if !should_validate_face_thumbnail(entity, deployment.files) {
            return ValidationResponse::Ok;
        }

        let avatars = entity
            .metadata
            .as_ref()
            .and_then(|m| m.get("avatars"))
            .and_then(|a| a.as_array());
        let avatars = match avatars {
            Some(a) => a,
            None => return ValidationResponse::Ok,
        };

        let mut errors: Vec<String> = Vec::new();
        for avatar in avatars {
            let hash = match face256_of(avatar) {
                Some(h) => h.to_string(),
                None => {
                    return ValidationResponse::fail(
                        "Couldn't find hash for face256 thumbnail file with name: 'face256'"
                            .to_string(),
                    );
                }
            };

            let already_stored = match self
                .external_calls
                .is_content_stored_already(std::slice::from_ref(&hash))
                .await
            {
                Ok(stored) => stored.get(&hash).copied().unwrap_or(false),
                Err(msg) => {
                    if !deployment.files.contains_key(&hash) {
                        return content_store_unavailable(&msg);
                    }
                    warn!(
                        hash,
                        error = %msg,
                        "content store unreadable; re-checking the uploaded face256 thumbnail \
                         instead of skipping it"
                    );
                    false
                }
            };
            if already_stored {
                continue;
            }

            match deployment.files.get(&hash) {
                Some(file) => match file.read().await {
                    Ok(buffer) => errors.extend(check_face256_thumbnail_image(&buffer)),
                    Err(e) => return upload_unreadable(&e),
                },
                None => {
                    return ValidationResponse::fail(format!(
                        "Couldn't find thumbnail file with hash: {hash}"
                    ));
                }
            }
        }

        ValidationResponse::from_errors(errors)
    }

    /// Upstream `createProfileImagesValidateFn`: every avatar names truthy
    /// face256 and body snapshots. Its hash check is moot here: uploads are
    /// keyed by their computed hash and a synced entity file is verified.
    fn validate_profile_images(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;
        if entity.entity_type != EntityType::Profile
            || !validates_up_to_adr290_optionality(entity, deployment.files, adr_timestamps::ADR_45)
        {
            return ValidationResponse::Ok;
        }
        let avatars = entity
            .metadata
            .as_ref()
            .and_then(|m| m.get("avatars"))
            .and_then(|a| a.as_array())
            .map_or(&[][..], Vec::as_slice);
        if avatars.iter().any(|a| !avatar_has_face_and_body(a)) {
            return ValidationResponse::fail(
                "Couldn't find hash for face or body thumbnails on profile metadata".to_string(),
            );
        }
        ValidationResponse::Ok
    }

    /// The profile rules after the images one, in upstream order.
    fn validate_profile_content(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;
        if entity.entity_type != EntityType::Profile {
            return ValidationResponse::Ok;
        }

        if entity.timestamp >= adr_timestamps::ADR_75
            && let Some(metadata) = &entity.metadata
            && let Err(msg) = validate_profile_wearable_urns(metadata, entity.timestamp)
        {
            return ValidationResponse::fail(msg);
        }

        if entity.timestamp >= adr_timestamps::ADR_74
            && let Some(metadata) = &entity.metadata
        {
            if let Err(msg) = validate_profile_emote_urns(metadata, entity.timestamp) {
                return ValidationResponse::fail(msg);
            }

            if !profile_has_emotes(metadata) {
                return ValidationResponse::fail(
                    "Profile must have emotes after ADR 74.".to_string(),
                );
            }
        }

        if let Some(metadata) = &entity.metadata
            && let Some(avatars) = metadata.get("avatars").and_then(|v| v.as_array())
        {
            let mut used_slots = HashSet::new();
            for avatar in avatars {
                if let Some(emotes) = avatar
                    .get("avatar")
                    .and_then(|a| a.get("emotes"))
                    .and_then(|e| e.as_array())
                {
                    for emote in emotes {
                        let Some(slot) = emote.get("slot").and_then(|s| s.as_f64()) else {
                            continue;
                        };
                        if !self.synced && slot.fract() != 0.0 {
                            return ValidationResponse::fail(format!(
                                "The slot {slot} of the emote must be an integer."
                            ));
                        }
                        if !used_slots.insert(slot_key(slot)) {
                            return ValidationResponse::fail(format!(
                                "Emote slot {slot} should not be repeated."
                            ));
                        }
                    }
                }
            }
        }

        if entity.timestamp >= adr_timestamps::ADR_232
            && let Some(metadata) = &entity.metadata
            && let Some(avatars) = metadata.get("avatars").and_then(|v| v.as_array())
        {
            for avatar in avatars {
                if let Some(wearables) = avatar
                    .get("avatar")
                    .and_then(|a| a.get("wearables"))
                    .and_then(|w| w.as_array())
                {
                    let unique: HashSet<&str> =
                        wearables.iter().filter_map(|v| v.as_str()).collect();
                    if unique.len() != wearables.len() {
                        return ValidationResponse::fail(
                            "Wearables should not be repeated.".to_string(),
                        );
                    }
                }
            }
        }

        if validates_up_to_adr290_optionality(entity, deployment.files, adr_timestamps::ADR_45) {
            let has_avatars = entity
                .metadata
                .as_ref()
                .and_then(|m| m.get("avatars"))
                .and_then(|a| a.as_array())
                .is_some_and(|a| !a.is_empty());
            let errors: Vec<String> = entity
                .content
                .iter()
                .filter_map(|c| match &entity.metadata {
                    Some(metadata) if has_avatars => {
                        (!corresponds_to_a_snapshot(&c.file, &c.hash, metadata)).then(|| {
                            format!(
                                "This file is not expected: '{}' or its hash is invalid: '{}'. \
                                 Please, include only valid snapshot files.",
                                c.file, c.hash
                            )
                        })
                    }
                    _ => Some("Entity is missing metadata or avatars".to_string()),
                })
                .collect();
            if !errors.is_empty() {
                return ValidationResponse::from_errors(errors);
            }
        }

        if validates_up_to_adr290_optionality(entity, deployment.files, adr_timestamps::ADR_158) {
            let has = |name: &str| entity.content.iter().any(|c| c.file.to_lowercase() == name);
            let errors: Vec<String> = ["body.png", "face256.png"]
                .into_iter()
                .filter(|name| !has(name))
                .map(|name| format!("Profile entity is missing file '{name}'"))
                .collect();
            if !errors.is_empty() {
                return ValidationResponse::from_errors(errors);
            }
        }

        // Upstream `entityShouldNotHaveContentFilesValidateFn` reports both.
        if entity.timestamp >= adr_timestamps::ADR_290_REJECTED {
            let mut errors = Vec::new();
            if !entity.content.is_empty() {
                errors.push(format!(
                    "Entity has content files when it should not: {}",
                    entity
                        .content
                        .iter()
                        .map(|c| c.file.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if deployment.files.len() > 1 {
                errors.push(format!(
                    "Entity has uploaded files when it should not: {}",
                    deployment
                        .files
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !errors.is_empty() {
                return ValidationResponse::from_errors(errors);
            }
        }

        ValidationResponse::Ok
    }

    fn validate_scene_content(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;
        if entity.entity_type != EntityType::Scene {
            return ValidationResponse::Ok;
        }

        let parcels_result =
            validate_scene_parcels_match_pointers(entity.metadata.as_ref(), &entity.pointers);
        if !parcels_result.is_ok() {
            return parcels_result;
        }

        if entity.timestamp >= adr_timestamps::ADR_173
            && let Some(metadata) = &entity.metadata
            && metadata.get("worldConfiguration").is_some()
        {
            return ValidationResponse::fail(
                "The scene.json contains a worldConfiguration section, which is \
                 not allowed for Genesis City scenes (see ADR-173: \
                 http://adr.decentraland.org/adr/ADR-173). Please remove it and \
                 try again."
                    .to_string(),
            );
        }

        if entity.timestamp >= adr_timestamps::ADR_236
            && let Some(metadata) = &entity.metadata
        {
            let result = validate_scene_navmap_thumbnail(metadata, &entity.content);
            if !result.is_ok() {
                return result;
            }
        }

        ValidationResponse::Ok
    }

    async fn validate_content(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;
        let files = &deployment.files;
        let mut errors = Vec::new();

        if !entity.content.is_empty() {
            let mut asked: HashSet<&str> = HashSet::new();
            let unresolved: Vec<String> = entity
                .content
                .iter()
                .map(|c| c.hash.as_str())
                .filter(|h| !files.contains_key(*h) && asked.insert(h))
                .map(str::to_string)
                .collect();

            let stored = if unresolved.is_empty() {
                HashMap::new()
            } else {
                match self
                    .external_calls
                    .is_content_stored_already(&unresolved)
                    .await
                {
                    Ok(stored) => stored,
                    Err(msg) => return content_store_unavailable(&msg),
                }
            };

            for cm in &entity.content {
                let is_uploaded = files.contains_key(&cm.hash);
                let is_stored = stored.get(&cm.hash).copied().unwrap_or(false);
                if !is_uploaded && !is_stored {
                    errors.push(format!(
                        "This hash is referenced in the entity but was not uploaded \
                         or previously available: {}",
                        cm.hash
                    ));
                }
            }
        }

        let entity_hashes: HashSet<&String> = entity.content.iter().map(|c| &c.hash).collect();
        for hash in files.keys() {
            if !entity_hashes.contains(hash) && *hash != entity.id {
                errors.push(format!(
                    "This hash was uploaded but is not referenced in the entity: {hash}"
                ));
            }
        }

        ValidationResponse::from_errors(errors)
    }

    fn validate_outfits_content(&self, deployment: &Deployment<'_>) -> ValidationResponse {
        let entity = &deployment.entity;
        if entity.entity_type != EntityType::Outfits {
            return ValidationResponse::Ok;
        }

        let metadata = match &entity.metadata {
            Some(m) => m,
            None => return ValidationResponse::Ok,
        };

        if let Some(outfits) = metadata.get("outfits").and_then(|v| v.as_array()) {
            let mut used_slots = HashSet::new();
            for outfit in outfits {
                if let Some(slot) = outfit.get("slot").and_then(|s| s.as_f64()) {
                    if !self.synced && slot.fract() != 0.0 {
                        return ValidationResponse::fail(
                            "Outfits slots are invalid, they must be integers".to_string(),
                        );
                    }
                    if !(0.0..=9.0).contains(&slot) {
                        return ValidationResponse::fail(
                            "Outfits slots are invalid, they must be between 0 and 9 \
                             inclusive"
                                .to_string(),
                        );
                    }
                    if !used_slots.insert(slot_key(slot)) {
                        return ValidationResponse::fail("Outfits slots are repeated".to_string());
                    }
                }
            }

            let has_extra = outfits.iter().any(|o| {
                o.get("slot")
                    .and_then(|s| s.as_f64())
                    .map(|s| s > 4.0)
                    .unwrap_or(false)
            });
            if has_extra {
                let names = metadata
                    .get("namesForExtraSlots")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                if names == 0 {
                    return ValidationResponse::fail(
                        "A name must be provided if extra slots are used, but none \
                         were provided."
                            .to_string(),
                    );
                }
            }
        }

        if entity.timestamp >= adr_timestamps::ADR_244 {
            let mut seen: HashSet<&str> = HashSet::new();
            let mut invalid_urns: Vec<&str> = Vec::new();
            let mut non_item_urns: Vec<&str> = Vec::new();
            let mut unlinked_third_party_urns: Vec<&str> = Vec::new();
            let wearables = metadata
                .get("outfits")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|o| o.get("outfit")?.get("wearables")?.as_array())
                .flatten()
                .filter_map(|w| w.as_str());
            for urn in wearables {
                if !seen.insert(urn) {
                    continue;
                }
                match super::urn::parse(urn) {
                    None => invalid_urns.push(urn),
                    Some(parsed) if parsed.is_collection_asset() => non_item_urns.push(urn),
                    Some(parsed)
                        if entity.timestamp >= adr_timestamps::THIRD_PARTY_ITEM_URN
                            && parsed.is_third_party_without_nft() =>
                    {
                        unlinked_third_party_urns.push(urn)
                    }
                    Some(_) => {}
                }
            }
            if !invalid_urns.is_empty() {
                return ValidationResponse::fail(format!(
                    "Invalid wearable pointer: ({})",
                    invalid_urns.join(", ")
                ));
            }
            if !non_item_urns.is_empty() {
                return ValidationResponse::fail(format!(
                    "Wearable pointers {} should be items, not assets. \
                     The URNs must include the tokenId.",
                    non_item_urns.join(", ")
                ));
            }
            if !unlinked_third_party_urns.is_empty() {
                return ValidationResponse::fail(format!(
                    "Wearable pointers {} should be third-party items linked to an NFT. \
                     The URNs must include the NFT network, contract and tokenId.",
                    unlinked_third_party_urns.join(", ")
                ));
            }
        }

        ValidationResponse::Ok
    }

    async fn validate_access(
        &self,
        deployment: &Deployment<'_>,
        deadline: tokio::time::Instant,
    ) -> ValidationResponse {
        if self.access_passed {
            return ValidationResponse::Ok;
        }
        if self.ignore_blockchain_access {
            let deployer = deployment.deployer();
            let local = super::checker::validate_local_pointer_ownership(
                deployment,
                self.blockchain_checker,
                &deployer,
            );
            if !matches!(local, ValidationResponse::Ok) {
                return local;
            }
            warn!(
                entity_id = %deployment.entity.id,
                entity_type = %deployment.entity.entity_type,
                pointers = ?deployment.entity.pointers,
                "blockchain access checks bypassed for deployment \
                 \u{2014} should only be enabled on sync-only nodes"
            );
            return ValidationResponse::Ok;
        }

        let entity = &deployment.entity;
        let deployer = deployment.deployer();

        if entity.timestamp <= adr_timestamps::LEGACY_CONTENT_MIGRATION
            && self
                .blockchain_checker
                .is_address_owned_by_decentraland(&deployer)
        {
            return ValidationResponse::Ok;
        }

        if entity.entity_type == EntityType::Scene
            && entity.pointers.iter().any(|p| p.starts_with("default"))
        {
            return ValidationResponse::fail(
                "Scene pointers should only contain two integers separated by a comma, \
                 for example (10,10) or (120,-45)."
                    .to_string(),
            );
        }

        let checks = async {
            match entity.entity_type {
                EntityType::Scene => {
                    validate_scene_access(deployment, self.blockchain_checker, &deployer).await
                }
                EntityType::Profile => {
                    validate_profile_access(deployment, self.blockchain_checker, &deployer).await
                }
                EntityType::Store => validate_store_access(deployment, &deployer),
                EntityType::Wearable | EntityType::Emote => {
                    let single = self
                        .synced
                        .then(|| super::collections_v1::single_item_pointer(&entity.pointers))
                        .flatten();
                    match single {
                        Some(pointer) => {
                            let one = Entity {
                                pointers: vec![pointer],
                                ..(*entity).clone()
                            };
                            let view = Deployment {
                                entity: &one,
                                ..*deployment
                            };
                            validate_item_access(&view, self.blockchain_checker, &deployer).await
                        }
                        None => {
                            validate_item_access(deployment, self.blockchain_checker, &deployer)
                                .await
                        }
                    }
                }
                EntityType::Outfits => {
                    validate_outfits_access(deployment, self.blockchain_checker, &deployer).await
                }
            }
        };
        tokio::time::timeout_at(deadline, checks)
            .await
            .unwrap_or_else(|_| {
                ValidationResponse::unavailable(format!(
                    "Error checking access: the checks did not finish within {} s",
                    self.access_budget().as_secs()
                ))
            })
    }
}

/// Representations must exist and each listed file must be in `content`.
fn validate_representation_contents(
    entity: &Entity,
    representations: Option<&serde_json::Value>,
    kind: &str,
) -> ValidationResponse {
    let reps = match representations.and_then(|r| r.as_array()) {
        Some(reps) if !reps.is_empty() => reps,
        _ => return ValidationResponse::fail(format!("No {kind} representations found")),
    };
    if entity.content.is_empty() {
        return ValidationResponse::fail("No content found".to_string());
    }
    let entity_files: HashSet<&str> = entity.content.iter().map(|c| c.file.as_str()).collect();
    for rep in reps {
        if let Some(contents) = rep.get("contents").and_then(|c| c.as_array()) {
            for content_ref in contents {
                if let Some(file_name) = content_ref.as_str()
                    && !entity_files.contains(file_name)
                {
                    return ValidationResponse::fail(format!(
                        "Representation content: '{file_name}' is not one \
                         of the content files"
                    ));
                }
            }
        }
    }
    ValidationResponse::Ok
}

/// ADR-287 after the metadata schema, which already checked the social emote
/// shape and allows `null`: `startAnimation` and `outcomes` must be set.
fn validate_social_emote(data: Option<&serde_json::Value>) -> ValidationResponse {
    let Some(data) = data else {
        return ValidationResponse::fail("No emote data found".to_string());
    };
    if !super::metadata_schema::SOCIAL_EMOTE_PROPS
        .iter()
        .any(|p| data.get(p).is_some())
    {
        return ValidationResponse::Ok;
    }
    if data["startAnimation"].is_null() {
        return ValidationResponse::fail(
            "Some properties of StartAnimation are not valid".to_string(),
        );
    }
    if data["outcomes"].is_null() {
        return ValidationResponse::fail("Outcomes array cannot be empty".to_string());
    }
    ValidationResponse::Ok
}

/// ADR-45 hashing: from ADR-45 on, the entity id and every content hash are
/// IPFS v2 (`IPFSv2.validate`).
fn validate_ipfs_hashing(entity: &Entity) -> ValidationResponse {
    if entity.timestamp < adr_timestamps::ADR_45 {
        return ValidationResponse::Ok;
    }
    let errors: Vec<String> = std::iter::once(&entity.id)
        .chain(entity.content.iter().map(|cm| &cm.hash))
        .filter(|hash| !is_ipfs_v2(hash))
        .map(|hash| format!("This hash '{hash}' is not valid. It should be IPFS v2 format."))
        .collect();
    ValidationResponse::from_errors(errors)
}

fn validate_scene_navmap_thumbnail(
    metadata: &serde_json::Value,
    content: &[ContentMapping],
) -> ValidationResponse {
    match navmap_thumbnail_error(metadata, |file| content.iter().any(|c| c.file == file)) {
        Some(message) => ValidationResponse::fail(message),
        None => ValidationResponse::Ok,
    }
}

#[cfg(test)]
#[path = "store_fault_tests.rs"]
mod store_fault_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn thumbnail_metadata(value: serde_json::Value) -> serde_json::Value {
        serde_json::json!({ "display": { "navmapThumbnail": value } })
    }

    fn scene_content(file: &str) -> Vec<ContentMapping> {
        vec![ContentMapping {
            file: file.to_string(),
            hash: "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenosa7776".to_string(),
        }]
    }

    fn failure(result: ValidationResponse) -> Vec<String> {
        result.errors().map(<[String]>::to_vec).unwrap_or_default()
    }

    fn item(content: Vec<ContentMapping>) -> Entity {
        Entity {
            id: "bafkreiitem".into(),
            entity_type: EntityType::Wearable,
            pointers: vec!["urn:decentraland:matic:collections-v2:0xabc:0".into()],
            timestamp: 1_780_000_000_000,
            content,
            version: "v3".into(),
            metadata: None,
        }
    }

    #[test]
    fn ipfs_hashing_takes_any_ipfs_v2_hash_and_nothing_else() {
        let other_prefix = format!("bag{}", "a".repeat(56));
        let bafy_prefix_only = "bafybogus".to_string();
        let mut entity = item(vec![ContentMapping {
            file: "model.glb".into(),
            hash: other_prefix.clone(),
        }]);
        entity.id = "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenosa7776".into();
        assert!(validate_ipfs_hashing(&entity).is_ok());

        entity.content.push(ContentMapping {
            file: "thumbnail.png".into(),
            hash: bafy_prefix_only.clone(),
        });
        assert_eq!(
            failure(validate_ipfs_hashing(&entity)),
            vec![format!(
                "This hash '{bafy_prefix_only}' is not valid. It should be IPFS v2 format."
            )]
        );

        entity.timestamp = adr_timestamps::ADR_45 - 1;
        assert!(validate_ipfs_hashing(&entity).is_ok());
    }

    #[test]
    fn items_without_representations_or_content_are_rejected() {
        let reps = serde_json::json!([{"contents": ["thumbnail.png"]}]);
        for kind in ["wearable", "emote"] {
            for missing in [
                None,
                Some(serde_json::json!([])),
                Some(serde_json::json!({})),
            ] {
                assert_eq!(
                    failure(validate_representation_contents(
                        &item(scene_content("thumbnail.png")),
                        missing.as_ref(),
                        kind
                    )),
                    vec![format!("No {kind} representations found")]
                );
            }
            assert_eq!(
                failure(validate_representation_contents(
                    &item(vec![]),
                    Some(&reps),
                    kind
                )),
                vec!["No content found".to_string()]
            );
        }
        assert!(
            validate_representation_contents(
                &item(scene_content("thumbnail.png")),
                Some(&reps),
                "wearable"
            )
            .is_ok()
        );
    }

    fn social(outcomes: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "startAnimation": {"loop": true, "Armature": {"animation": "Start"}},
            "randomizeOutcomes": false,
            "outcomes": outcomes
        })
    }

    fn outcome() -> serde_json::Value {
        serde_json::json!({"title": "t", "loop": false, "clips": {"Armature": {"animation": "a"}}})
    }

    #[test]
    fn social_emotes_need_their_start_animation_and_outcomes_set() {
        let plain = serde_json::json!({"loop": false});
        assert!(validate_social_emote(Some(&plain)).is_ok());
        assert!(validate_social_emote(Some(&social(serde_json::json!([outcome()])))).is_ok());
        assert_eq!(
            failure(validate_social_emote(None)),
            vec!["No emote data found".to_string()]
        );
        let mut no_start = social(serde_json::json!([outcome()]));
        no_start["startAnimation"] = serde_json::Value::Null;
        assert_eq!(
            failure(validate_social_emote(Some(&no_start))),
            vec!["Some properties of StartAnimation are not valid".to_string()]
        );
        assert_eq!(
            failure(validate_social_emote(Some(&social(
                serde_json::Value::Null
            )))),
            vec!["Outcomes array cannot be empty".to_string()]
        );
    }

    #[test]
    fn navmap_thumbnail_relative_embedded_path_passes() {
        let metadata = thumbnail_metadata(serde_json::json!("thumbnail.png"));
        assert!(
            validate_scene_navmap_thumbnail(&metadata, &scene_content("thumbnail.png")).is_ok()
        );
    }

    #[test]
    fn navmap_thumbnail_absent_null_or_empty_is_skipped() {
        let absent = serde_json::json!({ "display": {} });
        assert!(validate_scene_navmap_thumbnail(&absent, &scene_content("x.png")).is_ok());
        let null = thumbnail_metadata(serde_json::Value::Null);
        assert!(validate_scene_navmap_thumbnail(&null, &scene_content("x.png")).is_ok());
        let empty = thumbnail_metadata(serde_json::json!(""));
        assert!(validate_scene_navmap_thumbnail(&empty, &scene_content("x.png")).is_ok());
        let falsey = thumbnail_metadata(serde_json::json!(false));
        assert!(validate_scene_navmap_thumbnail(&falsey, &scene_content("x.png")).is_ok());
        let zero = thumbnail_metadata(serde_json::json!(0));
        assert!(validate_scene_navmap_thumbnail(&zero, &scene_content("x.png")).is_ok());
        let zero_float = thumbnail_metadata(serde_json::json!(0.0));
        assert!(validate_scene_navmap_thumbnail(&zero_float, &scene_content("x.png")).is_ok());
    }

    #[test]
    fn navmap_thumbnail_array_value_is_rejected() {
        let payload = "https://x\"><script>alert(1)</script><meta name=\"y";
        let metadata = thumbnail_metadata(serde_json::json!([payload]));
        match validate_scene_navmap_thumbnail(&metadata, &scene_content("x.png")) {
            ValidationResponse::Failed { errors } => {
                assert_eq!(errors.len(), 1);
                assert!(
                    errors[0].starts_with("Scene thumbnail '[")
                        && errors[0].ends_with(
                            "must be a relative path to a file included in the deployment."
                        ),
                    "unexpected error: {}",
                    errors[0]
                );
            }
            other => panic!("array-valued navmapThumbnail must be rejected, got {other}"),
        }
    }

    #[test]
    fn navmap_thumbnail_number_value_is_rejected() {
        let metadata = thumbnail_metadata(serde_json::json!(5));
        match validate_scene_navmap_thumbnail(&metadata, &scene_content("x.png")) {
            ValidationResponse::Failed { errors } => {
                assert_eq!(
                    errors[0],
                    "Scene thumbnail '5' must be a relative path to a \
                     file included in the deployment."
                );
            }
            other => panic!("number-valued navmapThumbnail must be rejected, got {other}"),
        }
    }

    #[test]
    fn navmap_thumbnail_absolute_or_scheme_paths_are_rejected() {
        for bad in ["https://evil.com/x.png", "//evil.com/x.png", "/etc/passwd"] {
            let metadata = thumbnail_metadata(serde_json::json!(bad));
            assert!(
                !validate_scene_navmap_thumbnail(&metadata, &scene_content(bad)).is_ok(),
                "{bad} must be rejected"
            );
        }
    }

    #[test]
    fn navmap_thumbnail_missing_from_content_is_rejected() {
        let metadata = thumbnail_metadata(serde_json::json!("thumbnail.png"));
        match validate_scene_navmap_thumbnail(&metadata, &scene_content("other.png")) {
            ValidationResponse::Failed { errors } => {
                assert_eq!(
                    errors[0],
                    "Scene thumbnail 'thumbnail.png' must be a file included in \
                     the deployment."
                );
            }
            other => panic!("non-embedded thumbnail must be rejected, got {other}"),
        }
    }

    mod access_deadline {
        use super::super::super::checker::BlockchainLayer;
        use super::super::super::error::{PermissionResult, ValidatorError};
        use super::*;
        use crate::auth::{AuthLink, AuthLinkType};

        const OWNER: &str = "0xaaaa000000000000000000000000000000000001";

        /// Answers the signature step after its delay; holds no content.
        struct Calls(Duration, std::sync::atomic::AtomicUsize);

        impl Calls {
            fn new(delay: Duration) -> Self {
                Self(delay, std::sync::atomic::AtomicUsize::new(0))
            }
        }

        #[async_trait]
        impl ExternalCalls for Calls {
            async fn is_content_stored_already(
                &self,
                hashes: &[String],
            ) -> Result<HashMap<String, bool>, String> {
                Ok(hashes.iter().map(|h| (h.clone(), false)).collect())
            }

            async fn fetch_content_file_size(&self, _hash: &str) -> Result<Option<usize>, String> {
                Ok(None)
            }

            async fn validate_signature(
                &self,
                _entity_id: &str,
                _auth_chain: &AuthChain,
                _timestamp: Timestamp,
            ) -> Result<(), AuthError> {
                self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                tokio::time::sleep(self.0).await;
                Ok(())
            }
        }

        /// Grants every name and item after `0`; with `1` set, the items are
        /// too many to check.
        struct SlowChecker(Duration, bool);

        #[async_trait]
        impl BlockchainChecker for SlowChecker {
            async fn check_land_access(
                &self,
                _eth_address: &str,
                _parcels: &[(i32, i32)],
                _timestamp: Timestamp,
            ) -> Result<Vec<bool>, ValidatorError> {
                unreachable!()
            }

            async fn check_names_ownership(
                &self,
                _eth_address: &str,
                _names: &[String],
                _timestamp: Timestamp,
            ) -> Result<PermissionResult, ValidatorError> {
                tokio::time::sleep(self.0).await;
                Ok(PermissionResult::ok())
            }

            async fn check_items_ownership(
                &self,
                _eth_address: &str,
                _urns: &[String],
                _timestamp: Timestamp,
            ) -> Result<PermissionResult, ValidatorError> {
                tokio::time::sleep(self.0).await;
                if self.1 {
                    return Err(ValidatorError::TooManyItems("201 distinct items".into()));
                }
                Ok(PermissionResult::ok())
            }

            async fn check_collection_access(
                &self,
                _eth_address: &str,
                _contract_address: &str,
                _item_id: &str,
                _deployment: &Deployment<'_>,
                _timestamp: Timestamp,
                _layer: BlockchainLayer,
            ) -> Result<bool, ValidatorError> {
                unreachable!()
            }

            async fn check_third_party_access(
                &self,
                _asset_urn: &str,
                _entity: &Entity,
                _deployment: &Deployment<'_>,
                _timestamp: Timestamp,
            ) -> Result<bool, ValidatorError> {
                unreachable!()
            }

            fn is_address_owned_by_decentraland(&self, _address: &str) -> bool {
                false
            }
        }

        fn profile() -> OwnedDeployment {
            let mut d = OwnedDeployment::new(Entity {
                id: "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenosa7776".into(),
                entity_type: EntityType::Profile,
                pointers: vec![OWNER.to_string()],
                // No face thumbnail is required in this window.
                timestamp: adr_timestamps::ADR_290_OPTIONAL + 1000,
                content: vec![],
                version: "v3".into(),
                metadata: Some(serde_json::json!({ "avatars": [{
                    "name": "alice", "hasClaimedName": true, "description": "",
                    "ethAddress": OWNER, "version": 1, "tutorialStep": 0,
                    "avatar": {
                        "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseMale",
                        "eyes": { "color": { "r": 0.1, "g": 0.2, "b": 0.3 } },
                        "hair": { "color": { "r": 0.1, "g": 0.2, "b": 0.3 } },
                        "skin": { "color": { "r": 0.1, "g": 0.2, "b": 0.3 } },
                        "emotes": [],
                        "wearables": [
                            "urn:decentraland:matic:collections-v2:0x0002000000000000000000000000000000000001:0:1"
                        ]
                    }
                }]})),
            });
            d.auth_chain = vec![AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: OWNER.into(),
                signature: None,
            }];
            d
        }

        async fn access_with(delay: Duration) -> ValidationResponse {
            let d = profile();
            ContentValidator {
                external_calls: &Calls::new(Duration::ZERO),
                blockchain_checker: &SlowChecker(delay, false),
                ignore_blockchain_access: false,
                synced: false,
                access_passed: false,
            }
            .validate_access(&d.view(), tokio::time::Instant::now() + ACCESS_DEADLINE)
            .await
        }

        fn timed_out(response: &ValidationResponse) -> bool {
            matches!(response, ValidationResponse::Unavailable { errors }
                if errors == &["Error checking access: the checks did not finish within 30 s"])
        }

        #[tokio::test(start_paused = true)]
        async fn the_checks_of_one_entity_share_one_deadline() {
            assert!(access_with(Duration::from_secs(10)).await.is_ok());
            // Names and items each within the deadline, together past it.
            let late = access_with(ACCESS_DEADLINE * 2 / 3).await;
            assert!(timed_out(&late), "{late:?}");
        }

        #[tokio::test(start_paused = true)]
        async fn the_deadline_starts_before_the_signature_step() {
            let d = profile();
            let validate = |signature: Duration, staging: bool| {
                let d = &d;
                async move {
                    let calls = Calls::new(signature);
                    let validator = ContentValidator {
                        external_calls: &calls,
                        blockchain_checker: &SlowChecker(ACCESS_DEADLINE / 4, false),
                        ignore_blockchain_access: false,
                        synced: false,
                        access_passed: false,
                    };
                    if staging {
                        validator.validate_staging(&d.view(), true).await
                    } else {
                        validator.validate(&d.view()).await
                    }
                }
            };
            for staging in [false, true] {
                let quick = validate(Duration::from_secs(1), staging).await;
                assert!(quick.is_ok(), "{quick:?}");
                let late = validate(ACCESS_DEADLINE * 2 / 3, staging).await;
                assert!(timed_out(&late), "{late:?}");
            }
        }

        #[tokio::test]
        async fn both_paths_short_circuit_prefix_errors_before_the_signature() {
            let calls = Calls::new(Duration::ZERO);
            let checker = SlowChecker(Duration::ZERO, false);
            let validator = ContentValidator {
                external_calls: &calls,
                blockchain_checker: &checker,
                ignore_blockchain_access: true,
                synced: false,
                access_passed: false,
            };
            let mut d = profile();
            d.entity.pointers.clear();
            d.entity.id = "invalid-hash".into();
            d.entity.metadata = None;
            d.entity.version = "v2".into();
            for stage in 0..4 {
                let full = validator.validate(&d.view()).await.into_result();
                let staging = validator
                    .validate_staging(&d.view(), false)
                    .await
                    .into_result();
                assert_eq!(full, staging);
                assert!(full.is_err());
                assert_eq!(calls.1.load(std::sync::atomic::Ordering::SeqCst), 0);
                match stage {
                    0 => d.entity.pointers = vec![OWNER.into()],
                    1 => d.entity.id = profile().entity.id,
                    2 => d.entity.metadata = profile().entity.metadata,
                    _ => d.entity.version = "v3".into(),
                }
            }
            assert!(validator.validate(&d.view()).await.is_ok());
            assert!(validator.validate_staging(&d.view(), false).await.is_ok());
            assert_eq!(calls.1.load(std::sync::atomic::Ordering::SeqCst), 2);
        }

        /// A profile whose first avatar has emotes in the given slots.
        fn profile_with_emote_slots(slots: &str) -> OwnedDeployment {
            let mut d = profile();
            let emotes: Vec<serde_json::Value> = slots
                .split(',')
                .enumerate()
                .map(|(i, slot)| {
                    serde_json::from_str(&format!(
                        r#"{{"slot": {slot}, "urn": "urn:decentraland:off-chain:base-emotes:e{i}"}}"#
                    ))
                    .unwrap()
                })
                .collect();
            d.entity.metadata.as_mut().unwrap()["avatars"][0]["avatar"]["emotes"] =
                serde_json::Value::Array(emotes);
            d
        }

        /// An outfits entity with outfits in the given slots.
        fn outfits_with_slots(slots: &str, names: &str) -> OwnedDeployment {
            let outfits: Vec<String> = slots
                .split(',')
                .map(|slot| format!(r#"{{"slot": {slot}, "outfit": {{"wearables": []}}}}"#))
                .collect();
            let metadata = format!(
                r#"{{"outfits": [{}], "namesForExtraSlots": [{names}]}}"#,
                outfits.join(",")
            );
            let mut d = profile();
            d.entity.entity_type = EntityType::Outfits;
            d.entity.pointers = vec![format!("{OWNER}:outfits")];
            d.entity.metadata = Some(serde_json::from_str(&metadata).unwrap());
            d
        }

        /// Whether the profile or outfits content rules pass for `d`.
        fn slots_pass(d: &OwnedDeployment, synced: bool) -> bool {
            let calls = Calls::new(Duration::ZERO);
            let checker = SlowChecker(Duration::ZERO, false);
            let validator = ContentValidator {
                external_calls: &calls,
                blockchain_checker: &checker,
                ignore_blockchain_access: false,
                synced,
                access_passed: false,
            };
            match d.entity.entity_type {
                EntityType::Outfits => validator.validate_outfits_content(&d.view()),
                _ => validator.validate_profile_content(&d.view()),
            }
            .is_ok()
        }

        #[test]
        fn emote_slots_are_judged_by_their_numeric_value_in_every_json_form() {
            for synced in [false, true] {
                assert!(slots_pass(&profile_with_emote_slots("0,1.0,9e0"), synced));
                for slots in ["1e1", "-1.0", "1,1.0"] {
                    assert!(
                        !slots_pass(&profile_with_emote_slots(slots), synced),
                        "emote slots {slots}"
                    );
                }
            }
            // Fractional slots are refused on a new deploy; a peer's entity keeps the old reading.
            assert!(!slots_pass(&profile_with_emote_slots("1.5"), false));
            assert!(slots_pass(&profile_with_emote_slots("1.5"), true));
            // `-0.0` and `0` are one slot.
            assert!(!slots_pass(&profile_with_emote_slots("0,-0.0"), false));
            // Slots are unique across every avatar of the profile.
            let two_avatars = |first: &str, second: &str| {
                let mut d = profile_with_emote_slots(first);
                let metadata = d.entity.metadata.as_mut().unwrap();
                let mut avatar = metadata["avatars"][0].clone();
                avatar["avatar"]["emotes"] = profile_with_emote_slots(second)
                    .entity
                    .metadata
                    .unwrap()["avatars"][0]["avatar"]["emotes"]
                    .clone();
                metadata["avatars"].as_array_mut().unwrap().push(avatar);
                d
            };
            assert!(slots_pass(&two_avatars("1", "2.0"), false));
            assert!(!slots_pass(&two_avatars("1", "1.0"), false));
        }

        /// The profile content rules' verdict on `d`.
        fn profile_content(d: &OwnedDeployment) -> ValidationResponse {
            let calls = Calls::new(Duration::ZERO);
            let checker = SlowChecker(Duration::ZERO, false);
            ContentValidator {
                external_calls: &calls,
                blockchain_checker: &checker,
                ignore_blockchain_access: false,
                synced: true,
                access_passed: false,
            }
            .validate_profile_content(&d.view())
        }

        fn errors(response: ValidationResponse) -> Vec<String> {
            response
                .errors()
                .map(<[String]>::to_vec)
                .unwrap_or_default()
        }

        #[test]
        fn profiles_after_adr_74_need_an_avatar_with_emotes() {
            let with_emotes = |timestamp, emotes: &[serde_json::Value]| {
                let mut d = profile();
                d.entity.timestamp = timestamp;
                let metadata = d.entity.metadata.as_mut().unwrap();
                let avatar = metadata["avatars"][0].clone();
                metadata["avatars"] = emotes
                    .iter()
                    .map(|e| {
                        let mut a = avatar.clone();
                        a["avatar"]["emotes"] = e.clone();
                        a
                    })
                    .collect();
                d
            };
            let null = serde_json::Value::Null;
            let empty = serde_json::json!([]);
            let refused = vec!["Profile must have emotes after ADR 74.".to_string()];
            for ts in [
                adr_timestamps::ADR_74,
                adr_timestamps::ADR_290_OPTIONAL + 1000,
            ] {
                assert_eq!(
                    errors(profile_content(&with_emotes(
                        ts,
                        std::slice::from_ref(&null)
                    ))),
                    refused
                );
                let both_null = with_emotes(ts, &[null.clone(), null.clone()]);
                assert_eq!(errors(profile_content(&both_null)), refused);
                let mut absent = with_emotes(ts, std::slice::from_ref(&empty));
                absent.entity.metadata.as_mut().unwrap()["avatars"][0]["avatar"]
                    .as_object_mut()
                    .unwrap()
                    .remove("emotes");
                assert_eq!(errors(profile_content(&absent)), refused);
                assert!(profile_content(&with_emotes(ts, std::slice::from_ref(&empty))).is_ok());
                let one_empty = with_emotes(ts, &[null.clone(), empty.clone()]);
                assert!(profile_content(&one_empty).is_ok());
            }
            assert!(profile_content(&with_emotes(adr_timestamps::ADR_74 - 1, &[null])).is_ok());
        }

        /// A profile at `timestamp` with face256 `QmF` and body `QmB`
        /// snapshots and the given `(file, hash)` content.
        fn profile_with_files(timestamp: Timestamp, content: &[(&str, &str)]) -> OwnedDeployment {
            let mut d = profile();
            d.entity.timestamp = timestamp;
            d.entity.metadata.as_mut().unwrap()["avatars"][0]["avatar"]["snapshots"] =
                serde_json::json!({ "face256": "QmF", "body": "QmB" });
            d.entity.content = content
                .iter()
                .map(|(file, hash)| ContentMapping {
                    file: file.to_string(),
                    hash: hash.to_string(),
                })
                .collect();
            d
        }

        const SNAPSHOT_FILES: [(&str, &str); 2] = [("body.png", "QmB"), ("face256.png", "QmF")];

        #[test]
        fn pre_adr_290_profile_files_must_be_avatar_snapshots() {
            let not_expected = |file: &str, hash: &str| {
                format!(
                    "This file is not expected: '{file}' or its hash is invalid: '{hash}'. \
                     Please, include only valid snapshot files."
                )
            };
            let with_extra = |ts, extra: (&'static str, &'static str)| {
                let mut files = SNAPSHOT_FILES.to_vec();
                files.push(extra);
                profile_with_files(ts, &files)
            };
            for ts in [
                adr_timestamps::ADR_158,
                adr_timestamps::ADR_290_OPTIONAL - 1,
                adr_timestamps::ADR_290_OPTIONAL + 1000,
            ] {
                assert!(profile_content(&profile_with_files(ts, &SNAPSHOT_FILES)).is_ok());
                assert_eq!(
                    errors(profile_content(&with_extra(ts, ("other.png", "QmO")))),
                    vec![not_expected("other.png", "QmO")]
                );
                // Only the last extension is dropped, and the hash must match the snapshot.
                assert!(profile_content(&with_extra(ts, ("body", "QmB"))).is_ok());
                assert!(profile_content(&with_extra(ts, ("body.jpg", "QmB"))).is_ok());
                assert!(!profile_content(&with_extra(ts, ("body.png.bak", "QmB"))).is_ok());
                assert!(!profile_content(&with_extra(ts, ("face256.png", "QmB"))).is_ok());
            }
            // From ADR-45, before the mandatory files of ADR-158.
            let stray = [("other.png", "QmO"), ("face256.png", "QmX")];
            assert_eq!(
                errors(profile_content(&profile_with_files(
                    adr_timestamps::ADR_45,
                    &stray
                ))),
                vec![
                    not_expected("other.png", "QmO"),
                    not_expected("face256.png", "QmX")
                ]
            );
            let mut no_avatars = profile_with_files(adr_timestamps::ADR_45, &stray);
            no_avatars.entity.metadata = Some(serde_json::json!({ "avatars": [] }));
            assert_eq!(
                errors(profile_content(&no_avatars)),
                vec!["Entity is missing metadata or avatars".to_string(); 2]
            );
            assert!(
                profile_content(&profile_with_files(adr_timestamps::ADR_45 - 1, &stray)).is_ok()
            );
        }

        #[test]
        fn pre_adr_290_profiles_must_upload_body_and_face() {
            let both_missing = vec![
                "Profile entity is missing file 'body.png'".to_string(),
                "Profile entity is missing file 'face256.png'".to_string(),
            ];
            let bare = |ts| {
                let mut d = profile();
                d.entity.timestamp = ts;
                d
            };
            for ts in [
                adr_timestamps::ADR_158,
                adr_timestamps::ADR_290_OPTIONAL - 1,
            ] {
                assert_eq!(errors(profile_content(&bare(ts))), both_missing);
                assert_eq!(
                    errors(profile_content(&profile_with_files(
                        ts,
                        &[("body.png", "QmB")]
                    ))),
                    vec!["Profile entity is missing file 'face256.png'".to_string()]
                );
            }
            assert!(profile_content(&bare(adr_timestamps::ADR_158 - 1)).is_ok());
            // In the optional window only a profile still carrying snapshots,
            // content or uploads is held to it.
            let optional = adr_timestamps::ADR_290_OPTIONAL + 1000;
            assert!(profile_content(&bare(optional)).is_ok());
            assert_eq!(
                errors(profile_content(&profile_with_files(optional, &[]))),
                both_missing
            );
            // File names are matched case-insensitively.
            let mut upper = profile_with_files(
                adr_timestamps::ADR_158,
                &[("Body.PNG", "QmB"), ("FACE256.png", "QmF")],
            );
            assert!(!profile_content(&upper).is_ok());
            upper.entity.metadata.as_mut().unwrap()["avatars"][0]["avatar"]["snapshots"] = serde_json::json!({ "face256": "QmF", "body": "QmB", "Body": "QmB", "FACE256": "QmF" });
            assert!(profile_content(&upper).is_ok());
        }

        /// The profile images rule's verdict on a profile at `timestamp`
        /// whose avatars carry the given `snapshots` values.
        fn profile_images(
            timestamp: Timestamp,
            snapshots: &[serde_json::Value],
        ) -> ValidationResponse {
            let mut d = profile();
            d.entity.timestamp = timestamp;
            let metadata = d.entity.metadata.as_mut().unwrap();
            let avatar = metadata["avatars"][0].clone();
            metadata["avatars"] = snapshots
                .iter()
                .map(|s| {
                    let mut a = avatar.clone();
                    a["avatar"]["snapshots"] = s.clone();
                    a
                })
                .collect();
            let calls = Calls::new(Duration::ZERO);
            let checker = SlowChecker(Duration::ZERO, false);
            ContentValidator {
                external_calls: &calls,
                blockchain_checker: &checker,
                ignore_blockchain_access: false,
                synced: true,
                access_passed: false,
            }
            .validate_profile_images(&d.view())
        }

        #[test]
        fn pre_adr_290_avatars_need_face_and_body_snapshots() {
            use serde_json::json;
            let missing = vec![
                "Couldn't find hash for face or body thumbnails on profile metadata".to_string(),
            ];
            let both = json!({ "face256": "QmF", "body": "QmB" });
            for ts in [
                adr_timestamps::ADR_45,
                adr_timestamps::ADR_290_OPTIONAL - 1,
                adr_timestamps::ADR_290_OPTIONAL + 1000,
                adr_timestamps::ADR_290_REJECTED - 1,
            ] {
                assert!(profile_images(ts, std::slice::from_ref(&both)).is_ok());
                assert!(
                    profile_images(ts, &[]).is_ok(),
                    "no avatars, nothing checked"
                );
                // Read with JS truthiness: "", 0, false and null are missing.
                for bad in [
                    json!({ "face256": "QmF" }),
                    json!({ "body": "QmB" }),
                    json!({ "face256": "", "body": "QmB" }),
                    json!({ "face256": "QmF", "body": "" }),
                    json!({ "face256": "QmF", "body": 0 }),
                    json!({ "face256": false, "body": "QmB" }),
                    json!({ "face256": "QmF", "body": null }),
                    json!("QmF"),
                ] {
                    assert_eq!(
                        errors(profile_images(ts, &[both.clone(), bad.clone()])),
                        missing,
                        "{ts} {bad}"
                    );
                }
                assert!(profile_images(ts, &[json!({ "face256": 1, "body": [] })]).is_ok());
            }
            // Before ADR-45, and from ADR-290 rejection, it does not run.
            assert!(profile_images(adr_timestamps::ADR_45 - 1, &[json!({ "body": "" })]).is_ok());
            assert!(
                profile_images(adr_timestamps::ADR_290_REJECTED, &[json!({ "body": "" })]).is_ok()
            );
            // In the optional window a profile without snapshots, content or
            // uploads is not held to it; one avatar with snapshots opts all in.
            let optional = adr_timestamps::ADR_290_OPTIONAL + 1000;
            assert!(profile_images(optional, &[json!(null)]).is_ok());
            assert_eq!(
                errors(profile_images(optional, &[both.clone(), json!(null)])),
                missing
            );
            assert_eq!(
                errors(profile_images(
                    adr_timestamps::ADR_290_OPTIONAL - 1,
                    &[json!(null)]
                )),
                missing
            );
        }

        /// The whole profile step's verdict on `d`.
        async fn profile_verdict(d: &OwnedDeployment) -> ValidationResponse {
            let calls = Calls::new(Duration::ZERO);
            let checker = SlowChecker(Duration::ZERO, false);
            ContentValidator {
                external_calls: &calls,
                blockchain_checker: &checker,
                ignore_blockchain_access: false,
                synced: false,
                access_passed: false,
            }
            .validate_profile(&d.view())
            .await
        }

        /// A PNG header of the given size, as the image check reads it.
        fn png(width: u32, height: u32) -> Vec<u8> {
            let mut buf = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
            buf.extend_from_slice(&13u32.to_be_bytes());
            buf.extend_from_slice(b"IHDR");
            buf.extend_from_slice(&width.to_be_bytes());
            buf.extend_from_slice(&height.to_be_bytes());
            buf.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
            buf.extend_from_slice(&0u32.to_be_bytes());
            buf.extend_from_slice(b"IEND");
            buf.extend_from_slice(&[0, 0, 0, 0]);
            buf
        }

        const WEARABLE: &str =
            "urn:decentraland:matic:collections-v2:0x0002000000000000000000000000000000000001:0:1";

        fn avatar_mut(d: &mut OwnedDeployment) -> &mut serde_json::Value {
            &mut d.entity.metadata.as_mut().unwrap()["avatars"][0]["avatar"]
        }

        #[tokio::test]
        async fn pre_adr_290_profile_rules_answer_in_upstream_order() {
            use serde_json::json;
            let mut d = profile_with_emote_slots("1,1");
            d.entity.timestamp = adr_timestamps::ADR_290_OPTIONAL - 1;
            avatar_mut(&mut d)["wearables"] = json!([WEARABLE, WEARABLE]);
            avatar_mut(&mut d)["snapshots"] = json!({ "face256": "QmF" });
            d.entity.content = vec![ContentMapping {
                file: "other.png".into(),
                hash: "QmO".into(),
            }];
            assert_eq!(
                errors(profile_verdict(&d).await),
                vec!["Couldn't find thumbnail file with hash: QmF".to_string()]
            );
            d.files.insert("QmF".into(), png(256, 256).into());
            assert_eq!(
                errors(profile_verdict(&d).await),
                vec![
                    "Couldn't find hash for face or body thumbnails on profile metadata"
                        .to_string()
                ]
            );
            avatar_mut(&mut d)["snapshots"]["body"] = json!("QmB");
            assert_eq!(
                errors(profile_verdict(&d).await),
                vec!["Emote slot 1 should not be repeated.".to_string()]
            );
            avatar_mut(&mut d)["emotes"][1]["slot"] = json!(2);
            assert_eq!(
                errors(profile_verdict(&d).await),
                vec!["Wearables should not be repeated.".to_string()]
            );
            avatar_mut(&mut d)["wearables"] = json!([WEARABLE]);
            assert_eq!(
                errors(profile_verdict(&d).await),
                vec![
                    "This file is not expected: 'other.png' or its hash is invalid: 'QmO'. \
                     Please, include only valid snapshot files."
                        .to_string()
                ]
            );
        }

        #[tokio::test]
        async fn the_profile_step_answers_before_the_content_cross_check() {
            use serde_json::json;
            let calls = Calls::new(Duration::ZERO);
            let checker = SlowChecker(Duration::ZERO, false);
            let validator = ContentValidator {
                external_calls: &calls,
                blockchain_checker: &checker,
                ignore_blockchain_access: true,
                synced: false,
                access_passed: false,
            };
            let (face, body, other, extra) = (
                "bafkreialsvt77jvpy673cnugp5ggnxfaalfncufweayuk3jbxskh3pelkm",
                "bafkreigreflbn4w3a36rgg2ywlhf2asebqlsd4skg5q5djpklcdcjkbjvi",
                "bafkreigxzm4duawatfhiylmawwdr4yezskltd3ygkup6nsl4yyif5t57i4",
                "bafkreicxp2hlkzqvax4dwm2bqic6owqv4qipd4a3zs5zjsfs2l2kas5fhm",
            );
            let mut d = profile();
            d.entity.timestamp = adr_timestamps::ADR_290_OPTIONAL - 1;
            avatar_mut(&mut d)["snapshots"] = json!({ "face256": face, "body": body });
            for hash in [face, body, other, extra] {
                d.files.insert(hash.into(), png(256, 256).into());
            }
            d.entity.content = [
                ("face256.png", face),
                ("body.png", body),
                ("other.png", other),
            ]
            .into_iter()
            .map(|(file, hash)| ContentMapping {
                file: file.into(),
                hash: hash.into(),
            })
            .collect();
            // `extra` would fail the later cross-check; the profile step answers first.
            assert_eq!(
                errors(validator.validate(&d.view()).await),
                vec![format!(
                    "This file is not expected: 'other.png' or its hash is invalid: '{other}'. \
                     Please, include only valid snapshot files."
                )]
            );
            d.entity.content.pop();
            d.files.remove(other);
            assert_eq!(
                errors(validator.validate(&d.view()).await),
                vec![format!(
                    "This hash was uploaded but is not referenced in the entity: {extra}"
                )]
            );
        }

        #[tokio::test]
        async fn rejected_profile_reports_content_and_uploads_last_and_together() {
            use serde_json::json;
            let mut d = profile_with_emote_slots("1,2");
            d.entity.timestamp = adr_timestamps::ADR_290_REJECTED;
            avatar_mut(&mut d)["wearables"] = json!([WEARABLE, WEARABLE]);
            avatar_mut(&mut d)["snapshots"] = json!({ "face256": "QmF", "body": "QmB" });
            d.entity.content = vec![ContentMapping {
                file: "face256.png".into(),
                hash: "QmF".into(),
            }];
            d.files.insert(d.entity.id.clone(), Vec::new().into());
            d.files.insert("QmF".into(), png(256, 256).into());
            assert_eq!(
                errors(profile_verdict(&d).await),
                vec!["Avatars must not have snapshots.".to_string()]
            );
            avatar_mut(&mut d)
                .as_object_mut()
                .unwrap()
                .remove("snapshots");
            assert_eq!(
                errors(profile_verdict(&d).await),
                vec!["Wearables should not be repeated.".to_string()]
            );
            avatar_mut(&mut d)["wearables"] = json!([WEARABLE]);
            let both = errors(profile_verdict(&d).await);
            assert_eq!(both.len(), 2, "{both:?}");
            assert_eq!(
                both[0],
                "Entity has content files when it should not: face256.png"
            );
            assert!(
                both[1].starts_with("Entity has uploaded files when it should not: "),
                "{both:?}"
            );
            d.entity.content.clear();
            assert_eq!(
                errors(profile_verdict(&d).await).len(),
                1,
                "uploads alone are still refused"
            );
            d.files.remove("QmF");
            assert!(profile_verdict(&d).await.is_ok());
        }

        #[test]
        fn outfit_slots_are_judged_by_their_numeric_value_in_every_json_form() {
            for synced in [false, true] {
                assert!(slots_pass(&outfits_with_slots("0,1.0,4e0", ""), synced));
                assert!(slots_pass(&outfits_with_slots("5.0", r#""alice""#), synced));
                for slots in ["10.0", "-1.0", "1,1e0", "5e0"] {
                    assert!(
                        !slots_pass(&outfits_with_slots(slots, ""), synced),
                        "outfit slots {slots}"
                    );
                }
            }
            // Fractional slots are refused on a new deploy; a peer's entity keeps the old reading.
            assert!(!slots_pass(&outfits_with_slots("1.5", ""), false));
            assert!(slots_pass(&outfits_with_slots("1.5", ""), true));
            assert!(!slots_pass(&outfits_with_slots("4.5", ""), true));
            // `-0.0` and `0` are one slot.
            assert!(!slots_pass(&outfits_with_slots("0,-0.0", ""), false));
        }

        #[test]
        fn outfit_wearables_must_resolve_to_items() {
            let calls = Calls::new(Duration::ZERO);
            let checker = SlowChecker(Duration::ZERO, false);
            let validator = ContentValidator {
                external_calls: &calls,
                blockchain_checker: &checker,
                ignore_blockchain_access: false,
                synced: false,
                access_passed: false,
            };
            let c = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
            let wearing = |urn: &str| {
                let mut d = profile();
                d.entity.entity_type = EntityType::Outfits;
                d.entity.pointers = vec![format!("{OWNER}:outfits")];
                d.entity.metadata = Some(serde_json::json!({
                    "outfits": [{"slot": 0, "outfit": {"wearables": [urn]}}],
                    "namesForExtraSlots": []
                }));
                d
            };
            let errors = |urn: &str| {
                validator
                    .validate_outfits_content(&wearing(urn).view())
                    .errors()
                    .map(<[String]>::to_vec)
                    .unwrap_or_default()
            };
            assert_eq!(
                errors("urn:foo:bar"),
                ["Invalid wearable pointer: (urn:foo:bar)"]
            );
            let asset = format!("urn:decentraland:matic:collections-v2:{c}:1?x:2");
            assert_eq!(
                errors(&asset),
                [format!(
                    "Wearable pointers {asset} should be items, not assets. \
                     The URNs must include the tokenId."
                )]
            );
            for urn in [
                format!("URN:decentraland:matic:collections-v2:{c}:1:5"),
                format!("urn:decentraland:matic:collections-v2:{c}"),
                "urn:decentraland:off-chain:base-avatars:BaseMale".to_string(),
            ] {
                assert!(errors(&urn).is_empty(), "{urn}");
            }
        }

        #[test]
        fn outfit_third_party_wearables_must_name_their_nft_from_the_cutoff() {
            let calls = Calls::new(Duration::ZERO);
            let checker = SlowChecker(Duration::ZERO, false);
            let validator = ContentValidator {
                external_calls: &calls,
                blockchain_checker: &checker,
                ignore_blockchain_access: false,
                synced: false,
                access_passed: false,
            };
            let errors = |urn: &str, timestamp: Timestamp| {
                let mut d = profile();
                d.entity.entity_type = EntityType::Outfits;
                d.entity.pointers = vec![format!("{OWNER}:outfits")];
                d.entity.timestamp = timestamp;
                d.entity.metadata = Some(serde_json::json!({
                    "outfits": [{"slot": 0, "outfit": {"wearables": [urn]}}],
                    "namesForExtraSlots": []
                }));
                validator
                    .validate_outfits_content(&d.view())
                    .errors()
                    .map(<[String]>::to_vec)
                    .unwrap_or_default()
            };
            let from = adr_timestamps::THIRD_PARTY_ITEM_URN;
            let unlinked = "urn:decentraland:matic:collections-thirdparty:tp:col:hat";
            assert_eq!(
                errors(unlinked, from),
                [format!(
                    "Wearable pointers {unlinked} should be third-party items linked to an NFT. \
                     The URNs must include the NFT network, contract and tokenId."
                )]
            );
            assert!(errors(unlinked, from - 1).is_empty());
            let linked = "urn:decentraland:matic:collections-thirdparty:tp:col:hat:matic:\
                          0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d:5";
            assert!(errors(linked, from).is_empty());
        }

        #[tokio::test]
        async fn outfits_pointer_rule_holds_with_chain_checks_ignored() {
            let other = "0xbbbb000000000000000000000000000000000002";
            let validator = &ContentValidator {
                external_calls: &Calls::new(Duration::ZERO),
                blockchain_checker: &SlowChecker(Duration::ZERO, false),
                ignore_blockchain_access: true,
                synced: false,
                access_passed: false,
            };
            let access = |pointers: Vec<String>| {
                let mut d = profile();
                d.entity.entity_type = EntityType::Outfits;
                d.entity.pointers = pointers;
                d.entity.metadata = None;
                async move {
                    validator
                        .validate_access(&d.view(), tokio::time::Instant::now() + ACCESS_DEADLINE)
                        .await
                }
            };
            let own = format!("{OWNER}:outfits");
            assert!(access(vec![own.clone()]).await.is_ok());
            let mixed_case = format!("0x{}:outfits", OWNER[2..].to_uppercase());
            assert!(access(vec![mixed_case]).await.is_ok());
            for pointers in [
                vec![own.clone(), format!("{other}:outfits")],
                vec![format!("{other}:outfits")],
                vec![format!("{OWNER}:profile")],
                vec!["outfits".to_string()],
                vec![format!("{OWNER}:outfits:extra")],
                vec!["0xzz:outfits".to_string()],
            ] {
                let response = access(pointers.clone()).await;
                assert!(!response.is_ok(), "{pointers:?} must be refused");
            }
        }

        #[tokio::test]
        async fn profile_pointer_rule_holds_with_chain_checks_ignored() {
            let other = "0xbbbb000000000000000000000000000000000002";
            let validator = &ContentValidator {
                external_calls: &Calls::new(Duration::ZERO),
                blockchain_checker: &SlowChecker(Duration::ZERO, false),
                ignore_blockchain_access: true,
                synced: false,
                access_passed: false,
            };
            let access = |pointers: Vec<String>, eth_address: &str| {
                let mut d = profile();
                d.entity.pointers = pointers;
                d.entity.timestamp = adr_timestamps::PROFILE_IDENTITY;
                d.entity.metadata.as_mut().unwrap()["avatars"][0]["ethAddress"] =
                    eth_address.into();
                async move {
                    validator
                        .validate_access(&d.view(), tokio::time::Instant::now() + ACCESS_DEADLINE)
                        .await
                }
            };
            assert!(access(vec![OWNER.into()], OWNER).await.is_ok());
            let mixed_case = format!("0x{}", OWNER[2..].to_uppercase());
            assert!(access(vec![mixed_case], OWNER).await.is_ok());
            for (pointers, eth_address) in [
                (vec![OWNER.to_string(), other.to_string()], OWNER),
                (vec![other.to_string()], OWNER),
                (vec!["default1".to_string()], OWNER),
                (vec!["not-an-address".to_string()], OWNER),
                (vec![OWNER.to_string()], other),
            ] {
                let response = access(pointers.clone(), eth_address).await;
                assert!(
                    !response.is_ok(),
                    "{pointers:?} with ethAddress {eth_address} must be refused"
                );
            }
        }

        #[tokio::test]
        async fn too_many_items_refuse_the_deploy() {
            let d = profile();
            let response = ContentValidator {
                external_calls: &Calls::new(Duration::ZERO),
                blockchain_checker: &SlowChecker(Duration::ZERO, true),
                ignore_blockchain_access: false,
                synced: false,
                access_passed: false,
            }
            .validate_access(&d.view(), tokio::time::Instant::now() + ACCESS_DEADLINE)
            .await;
            assert!(
                matches!(&response, ValidationResponse::Failed { errors }
                    if errors == &["Error checking item ownership: too many items to check: 201 distinct items"]),
                "{response:?}"
            );
        }
    }
}
