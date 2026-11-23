//! A visiting bot's profile deployment through the content server's own code, in this process:
//! the form's chain as the deployment route reads it, the entity file as the deployer parses and
//! names it, and the whole content validation a local deployment gets. Only the chain lookups are
//! stood in for, and the profile must need none: it wears base avatars and claims no name.

use std::collections::{BTreeMap, HashMap};

use async_trait::async_trait;
use catalyst_comms_e2e::identity::{Identity, now_ms};
use catalyst_comms_e2e::profile::profile_deployment;
use catalyst_content::create_entity::extract_auth_chain_from_fields;
use catalyst_content::validator::checker::{BlockchainChecker, BlockchainLayer};
use catalyst_content::validator::entity_parser::parse_entity_from_bytes;
use catalyst_content::validator::error::{PermissionResult, ValidatorError};
use catalyst_content::validator::types::{Deployment, Entity, Timestamp, UploadedFile};
use catalyst_content::validator::{ContentValidator, ExternalCalls};
use dcl_auth::{AuthChain, AuthError};

/// The node's own calls, as `Sources` makes them, without a store: nothing is stored yet.
struct Node;

#[async_trait]
impl ExternalCalls for Node {
    async fn is_content_stored_already(
        &self,
        hashes: &[String],
    ) -> Result<HashMap<String, bool>, String> {
        Ok(hashes.iter().map(|hash| (hash.clone(), false)).collect())
    }

    async fn fetch_content_file_size(&self, _hash: &str) -> Result<Option<usize>, String> {
        Ok(None)
    }

    async fn validate_signature(
        &self,
        entity_id: &str,
        auth_chain: &AuthChain,
        timestamp: Timestamp,
    ) -> Result<(), AuthError> {
        dcl_auth::verify_auth_chain_async(auth_chain, entity_id, Some(timestamp), None).await
    }
}

/// A chain no visiting profile may need: every lookup fails the test.
struct NoChain;

#[async_trait]
impl BlockchainChecker for NoChain {
    async fn check_land_access(
        &self,
        _eth_address: &str,
        parcels: &[(i32, i32)],
        _timestamp: Timestamp,
    ) -> Result<Vec<bool>, ValidatorError> {
        panic!("a profile asked for LAND: {parcels:?}")
    }

    async fn check_names_ownership(
        &self,
        _eth_address: &str,
        names: &[String],
        _timestamp: Timestamp,
    ) -> Result<PermissionResult, ValidatorError> {
        panic!("a visiting profile claims names: {names:?}")
    }

    async fn check_items_ownership(
        &self,
        _eth_address: &str,
        urns: &[String],
        _timestamp: Timestamp,
    ) -> Result<PermissionResult, ValidatorError> {
        panic!("a visiting profile wears items someone owns: {urns:?}")
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
        panic!("a profile asked for a collection")
    }

    async fn check_third_party_access(
        &self,
        _asset_urn: &str,
        _entity: &Entity,
        _deployment: &Deployment<'_>,
        _timestamp: Timestamp,
    ) -> Result<bool, ValidatorError> {
        panic!("a profile asked for a third party")
    }

    fn is_address_owned_by_decentraland(&self, _address: &str) -> bool {
        false
    }
}

#[tokio::test]
async fn a_visiting_profile_passes_the_content_servers_own_validation() {
    for index in [0usize, 1, 7, 19] {
        let identity = Identity::visitor("validation", index as u32);
        let deployment = profile_deployment(&identity, index, now_ms());

        // the route: the chain from the form's fields
        let fields: BTreeMap<String, String> = deployment.fields().into_iter().collect();
        let chain = extract_auth_chain_from_fields(&fields)
            .unwrap()
            .expect("the form carries a chain");
        let chain: AuthChain = serde_json::from_value(chain).unwrap();

        // the deployer: the file the entity id names, parsed as the entity
        assert_eq!(
            dcl_auth::hash_bytes_v1(&deployment.entity),
            fields["entityId"]
        );
        let entity = parse_entity_from_bytes(&deployment.entity, &fields["entityId"])
            .expect("the deployer parses the entity file");
        let files: HashMap<String, UploadedFile> = HashMap::from([(
            entity.id.clone(),
            UploadedFile::from(deployment.entity.clone()),
        )]);

        // the validation of a local deployment
        let verdict = ContentValidator {
            external_calls: &Node,
            blockchain_checker: &NoChain,
            ignore_blockchain_access: false,
            synced: false,
            access_passed: false,
        }
        .validate(&Deployment {
            entity: &entity,
            files: &files,
            auth_chain: &chain,
        })
        .await;
        assert!(verdict.is_ok(), "profile {index}: {verdict:?}");
    }
}

#[tokio::test]
async fn the_validation_refuses_a_profile_signed_for_another_wallet() {
    // the same check against drift in the other direction: it does refuse what it must
    let identity = Identity::visitor("validation", 0);
    let deployment = profile_deployment(&identity, 0, now_ms());
    let other = profile_deployment(&Identity::visitor("validation", 1), 0, now_ms());
    let entity = parse_entity_from_bytes(&deployment.entity, &deployment.entity_id).unwrap();
    let files: HashMap<String, UploadedFile> = HashMap::from([(
        entity.id.clone(),
        UploadedFile::from(deployment.entity.clone()),
    )]);
    let verdict = ContentValidator {
        external_calls: &Node,
        blockchain_checker: &NoChain,
        ignore_blockchain_access: false,
        synced: false,
        access_passed: false,
    }
    .validate(&Deployment {
        entity: &entity,
        files: &files,
        auth_chain: &other.auth_chain,
    })
    .await;
    assert!(!verdict.is_ok(), "{verdict:?}");
}
