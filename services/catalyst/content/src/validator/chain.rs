//! The live access checker: [`Sources`] as the [`BlockchainChecker`]. Every
//! chain-derived check of a local deploy goes to [`OnChainAccess`] (the
//! Checker contract and the item contracts over `ETH_RPC_URL` and
//! `RPC_ENDPOINT_POLYGON`, at the deploy's block or five minutes before it):
//!
//! | check | how |
//! | --- | --- |
//! | LAND rights of a scene | `checkLAND` per parcel (owner, operator, update operator, update manager, approved; of the LAND or its estate) |
//! | claimed names | `checkName` per name |
//! | owned wearables/emotes | `ownerOf` of each item token, `ownerOf`/`balanceOf` of each third-party item's NFT, plus the escrow lease overlay `shop.usage_grants` |
//! | collection access of a wearable/emote deploy | `validateWearables` with the ADR-32 hashes of the deployment; L1 items only by the Decentraland address |
//! | third-party items | `validateThirdParty` with the root the entity's merkle proof yields |
//!
//! A check that answers "no" is a rejection with the old text; a source that
//! is unset or fails is a [`ValidatorError`], which the access step turns into
//! `Unavailable`.

use std::collections::HashSet;

use async_trait::async_trait;
use tracing::warn;

use super::Sources;
use super::checker::{BlockchainChecker, BlockchainLayer};
use super::error::{PermissionResult, ValidatorError};
use super::types::*;
use crate::chain_index::graph::normalize_urn;

pub const DECENTRALAND_ADDRESS: &str = "0x1337e0507eb4ab47e08a179573ed4533d9e22a7b";

/// The form a URN is compared in: lowercase, L1 spelled `ethereum`.
fn urn_key(urn: &str) -> String {
    normalize_urn(&urn.to_lowercase())
}

/// A decimal token id without leading zeros.
fn token_key(token_id: &str) -> String {
    let trimmed = token_id.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// `(item urn key, token)` of a URN ending in a token id.
fn split_token(key: &str) -> Option<(String, String)> {
    let (item, token) = key.rsplit_once(':')?;
    (key.split(':').count() == 7
        && key.contains(":collections-")
        && token.bytes().all(|b| b.is_ascii_digit()))
    .then(|| (item.to_string(), token_key(token)))
}

/// The raw URNs and their keys, for the overlay's `urn = ANY`.
fn exact_forms<'a>(urns: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut out: Vec<String> = urns
        .flat_map(|u| {
            let key = urn_key(u);
            let item = split_token(&key).map(|(item, _)| item);
            [Some(u.clone()), Some(key), item].into_iter().flatten()
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

fn like_pattern(urn: &str) -> String {
    let esc = urn
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("{esc}:%")
}

/// Whether an active lease `(urn key, token)` covers `urn`: the URN itself,
/// a token under it, or the URN's own item and token.
fn lease_covers(urn: &str, grants: &[(String, Option<String>)]) -> bool {
    let key = urn_key(urn);
    let token = split_token(&key);
    grants.iter().any(|(g, t)| {
        *g == key
            || g.starts_with(&format!("{key}:"))
            || token
                .as_ref()
                .is_some_and(|(item, tok)| g == item && t.as_deref() == Some(tok.as_str()))
    })
}

impl Sources {
    /// The owner's active escrow leases (`shop.usage_grants`) on the
    /// requested URNs (`exact`, compared as stored) or under the requested
    /// `LIKE` patterns: `(urn key, token id)`. Bounded by the request; the
    /// grantee's partial index serves it (the urn clause is a filter).
    async fn granted(
        &self,
        owner: &str,
        exact: &[String],
        patterns: &[String],
    ) -> Result<Vec<(String, Option<String>)>, ValidatorError> {
        if exact.is_empty() && patterns.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT urn, token_id FROM shop.usage_grants \
             WHERE grantee_address = lower($1) AND status = 'active' \
               AND (urn = ANY($2) OR urn LIKE ANY($3))",
        )
        .bind(owner)
        .bind(exact)
        .bind(patterns)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ValidatorError::BlockchainQuery(format!("usage grants query failed: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|(urn, token)| (urn_key(&urn), token.map(|t| token_key(&t))))
            .collect())
    }
}

#[async_trait]
impl BlockchainChecker for Sources {
    async fn check_land_access(
        &self,
        eth_address: &str,
        parcels: &[(i32, i32)],
        timestamp: Timestamp,
    ) -> Result<Vec<bool>, ValidatorError> {
        self.access.land(eth_address, parcels, timestamp).await
    }

    async fn check_names_ownership(
        &self,
        eth_address: &str,
        names: &[String],
        timestamp: Timestamp,
    ) -> Result<PermissionResult, ValidatorError> {
        let owned = self.access.names(eth_address, names, timestamp).await?;
        Ok(PermissionResult::from_failing(
            names
                .iter()
                .zip(owned)
                .filter(|(_, o)| !o)
                .map(|(n, _)| n.clone())
                .collect(),
        ))
    }

    async fn check_items_ownership(
        &self,
        eth_address: &str,
        urns: &[String],
        timestamp: Timestamp,
    ) -> Result<PermissionResult, ValidatorError> {
        let owned = self.access.items(eth_address, urns, timestamp).await?;
        let missing: Vec<&String> = urns
            .iter()
            .zip(&owned)
            .filter(|(_, o)| !**o)
            .map(|(u, _)| u)
            .collect();
        if missing.is_empty() {
            return Ok(PermissionResult::ok());
        }
        let patterns: Vec<String> = missing
            .iter()
            .flat_map(|u| [like_pattern(u), like_pattern(&urn_key(u))])
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let grants = self
            .granted(
                eth_address,
                &exact_forms(missing.iter().copied()),
                &patterns,
            )
            .await?;
        Ok(PermissionResult::from_failing(
            missing
                .into_iter()
                .filter(|u| !lease_covers(u, &grants))
                .cloned()
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
        // L1 collections are deployed by the Decentraland address only.
        if layer == BlockchainLayer::L1 {
            return Ok(false);
        }
        let hashes = super::item_hash::deployment_hashes(deployment)
            .map(Vec::from)
            .unwrap_or_else(|| vec![String::new()]);
        self.access
            .collection_item(eth_address, contract_address, item_id, &hashes, timestamp)
            .await
    }

    async fn check_third_party_access(
        &self,
        asset_urn: &str,
        entity: &Entity,
        _deployment: &Deployment<'_>,
        timestamp: Timestamp,
    ) -> Result<bool, ValidatorError> {
        let Some(metadata) = &entity.metadata else {
            return Ok(false);
        };
        let tp_props: super::third_party::ThirdPartyProps =
            match serde_json::from_value(metadata.clone()) {
                Ok(p) => p,
                Err(e) => {
                    warn!(asset_urn, error = %e, "could not parse third-party metadata");
                    return Ok(false);
                }
            };
        let Some(tp_id) = super::third_party::get_third_party_id(asset_urn) else {
            warn!(asset_urn, "could not derive third-party id from urn");
            return Ok(false);
        };
        let proof = &tp_props.merkle_proof;
        let Some(siblings) = proof
            .proof
            .iter()
            .map(|p| super::merkle::decode_hash32(p))
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(false);
        };
        let root = super::merkle::generate_root(proof.index, &proof.entity_hash, &siblings);
        self.access.third_party_root(&tp_id, root, timestamp).await
    }

    fn is_address_owned_by_decentraland(&self, address: &str) -> bool {
        let lower = address.to_lowercase();
        lower == DECENTRALAND_ADDRESS
            || self
                .additional_dcl_address
                .as_deref()
                .is_some_and(|a| a.to_lowercase() == lower)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::land_authz::resolve::{DEPLOY_GRANTING_LEGS, operator_flags};
    use crate::land_authz::{LandOperators, LandRightLeg, ParcelPermissionFlags};

    #[test]
    fn urn_and_token_keys_follow_the_stored_spelling() {
        assert_eq!(
            urn_key("urn:decentraland:MAINNET:collections-v1:0xABC:Hat"),
            "urn:decentraland:ethereum:collections-v1:0xabc:hat"
        );
        assert_eq!(token_key("007"), "7");
        assert_eq!(token_key("000"), "0");
        assert_eq!(like_pattern("a_b%c"), "a\\_b\\%c:%");
        assert_eq!(
            split_token("urn:decentraland:matic:collections-v2:0xabc:0:007"),
            Some((
                "urn:decentraland:matic:collections-v2:0xabc:0".to_string(),
                "7".to_string()
            ))
        );
        assert_eq!(
            split_token("urn:decentraland:matic:collections-v2:0xabc:0"),
            None
        );
    }

    #[test]
    fn a_lease_covers_the_urn_its_tokens_and_its_own_token() {
        let item = "urn:decentraland:matic:collections-v2:0xabc:0";
        let grant =
            |urn: &str, token: Option<&str>| vec![(urn.to_string(), token.map(str::to_string))];
        assert!(lease_covers(item, &grant(item, None)));
        assert!(lease_covers(&format!("{item}:9"), &grant(item, Some("9"))));
        assert!(!lease_covers(&format!("{item}:8"), &grant(item, Some("9"))));
        assert!(lease_covers(
            "urn:decentraland:matic:collections-v2:0xabc",
            &grant(item, Some("9"))
        ));
        assert!(!lease_covers(
            "urn:decentraland:matic:collections-v2:0xdef:0",
            &grant(item, None)
        ));
    }

    #[test]
    fn operator_grants_each_leg() {
        let ops = LandOperators {
            operator: Some("0xAAA1".into()),
            update_operator: Some("0xbbb2".into()),
            update_managers: vec!["0xccc3".into()],
            approved_for_all: vec!["0xDDD4".into()],
        };
        assert!(operator_flags("0xaaa1", &ops).grants_deploy());
        assert!(operator_flags("0xBBB2", &ops).grants_deploy());
        assert!(operator_flags("0xCCC3", &ops).grants_deploy());
        assert!(operator_flags("0xddd4", &ops).grants_deploy());
        assert!(!operator_flags("0xeee5", &ops).grants_deploy());
    }

    #[test]
    fn operator_grants_denies_on_empty() {
        assert!(!operator_flags("0xaaa1", &LandOperators::default()).grants_deploy());
    }

    #[test]
    fn no_flags_denies() {
        assert!(!ParcelPermissionFlags::default().grants_deploy());
    }

    #[test]
    fn each_granting_leg_grants_alone() {
        for &leg in &DEPLOY_GRANTING_LEGS {
            let mut flags = ParcelPermissionFlags::default();
            match leg {
                LandRightLeg::Owner => flags.owner = true,
                LandRightLeg::Operator => flags.operator = true,
                LandRightLeg::UpdateOperator => flags.update_operator = true,
                LandRightLeg::UpdateManager => flags.update_manager = true,
                LandRightLeg::ApprovedForAll => flags.approved_for_all = true,
            }
            assert!(flags.grants_deploy(), "{leg:?} should grant deploy");
        }
    }

    #[test]
    fn serialized_shape_matches_lambdas_permissions_response() {
        let json = serde_json::to_string(&ParcelPermissionFlags::default()).unwrap();
        assert_eq!(
            json,
            "{\"owner\":false,\"operator\":false,\"updateOperator\":false,\"updateManager\":false,\"approvedForAll\":false}"
        );
    }

    #[test]
    fn legs_serialize_to_flag_keys() {
        for &leg in &DEPLOY_GRANTING_LEGS {
            let key = serde_json::to_value(leg).unwrap();
            let flags = serde_json::to_value(ParcelPermissionFlags::default()).unwrap();
            assert!(
                flags
                    .as_object()
                    .unwrap()
                    .contains_key(key.as_str().unwrap())
            );
        }
    }
}
