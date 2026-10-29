//! The deployment under validation and the validator's constants: the L1/L2
//! network spellings, the per-type size caps and the ADR activation
//! timestamps. The entity is the write path's own ([`crate::entity`]),
//! the auth chain the typed [`crate::auth::AuthChain`]; the old crate's
//! duplicates of both (and its string-typed link) are gone.

use std::collections::HashMap;

use crate::auth::AuthChain;
pub use crate::entity::{ContentMapping, Entity, EntityType};

pub type Timestamp = i64;

/// One deployment, borrowed from the write path: the parsed entity file,
/// the uploaded files keyed by their content hash (the entity file under
/// the entity id) and the signer's auth chain. The old `DeploymentToValidate`
/// owned copies of all three.
#[derive(Clone, Copy)]
pub struct Deployment<'a> {
    pub entity: &'a Entity,
    pub files: &'a HashMap<String, Vec<u8>>,
    pub auth_chain: &'a AuthChain,
}

impl Deployment<'_> {
    /// The deployer: the first link's payload (the old `owner_address`).
    pub fn deployer(&self) -> String {
        self.auth_chain
            .first()
            .map(|l| l.payload.clone())
            .unwrap_or_default()
    }
}

/// The tests' owned form of a [`Deployment`] (the old `DeploymentToValidate`).
#[cfg(test)]
pub(crate) struct OwnedDeployment {
    pub entity: Entity,
    pub files: HashMap<String, Vec<u8>>,
    pub auth_chain: AuthChain,
}

#[cfg(test)]
impl OwnedDeployment {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            files: HashMap::new(),
            auth_chain: AuthChain::new(),
        }
    }

    pub fn view(&self) -> Deployment<'_> {
        Deployment {
            entity: &self.entity,
            files: &self.files,
            auth_chain: &self.auth_chain,
        }
    }
}

pub const L1_NETWORKS: &[&str] = &["mainnet", "kovan", "rinkeby", "goerli", "sepolia"];

pub const L2_NETWORKS: &[&str] = &["matic", "mumbai", "amoy"];

pub fn is_l1_network(network: &str) -> bool {
    L1_NETWORKS.contains(&network) || network == "ethereum"
}

pub fn is_l2_network(network: &str) -> bool {
    L2_NETWORKS.contains(&network)
}

pub fn max_size_mb(entity_type: EntityType) -> Option<u64> {
    match entity_type {
        EntityType::Scene => Some(15),
        EntityType::Profile => Some(2),
        EntityType::Wearable => Some(3),
        EntityType::Store => Some(1),
        EntityType::Emote => Some(3),
        EntityType::Outfits => Some(1),
    }
}

pub const SKIN_MAX_SIZE_MB: u64 = 9;

pub mod adr_timestamps {
    use super::Timestamp;

    pub const ADR_45: Timestamp = 1_652_191_200_000;

    pub const ADR_74: Timestamp = 1_662_987_600_000;

    pub const ADR_75: Timestamp = 1_658_275_200_000;

    pub const ADR_158: Timestamp = 1_674_576_000_000;

    pub const ADR_173: Timestamp = 1_673_967_600_000;

    pub const ADR_232: Timestamp = 1_686_571_200_000;

    pub const ADR_236: Timestamp = 1_684_497_600_000;

    pub const ADR_244: Timestamp = 1_710_428_400_000;

    pub const ADR_290_OPTIONAL: Timestamp = 1_762_743_600_000;

    pub const ADR_290_REJECTED: Timestamp = ADR_290_OPTIONAL + 3 * 30 * 24 * 60 * 60 * 1000;

    pub const PROFILE_IDENTITY: Timestamp = 1_786_579_200_000;

    pub const LEGACY_CONTENT_MIGRATION: Timestamp = 1_582_167_600_000;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_caps_by_entity_type() {
        let caps: Vec<(EntityType, u64)> = [
            EntityType::Scene,
            EntityType::Profile,
            EntityType::Wearable,
            EntityType::Store,
            EntityType::Emote,
            EntityType::Outfits,
        ]
        .into_iter()
        .map(|t| (t, max_size_mb(t).unwrap()))
        .collect();
        assert_eq!(
            caps,
            vec![
                (EntityType::Scene, 15),
                (EntityType::Profile, 2),
                (EntityType::Wearable, 3),
                (EntityType::Store, 1),
                (EntityType::Emote, 3),
                (EntityType::Outfits, 1),
            ]
        );
        assert_eq!(SKIN_MAX_SIZE_MB, 9);
    }

    #[test]
    fn network_layers() {
        for l1 in ["mainnet", "ethereum", "sepolia", "goerli"] {
            assert!(is_l1_network(l1) && !is_l2_network(l1), "{l1}");
        }
        for l2 in ["matic", "mumbai", "amoy"] {
            assert!(is_l2_network(l2) && !is_l1_network(l2), "{l2}");
        }
        assert!(!is_l1_network("off-chain") && !is_l2_network("off-chain"));
    }

    #[test]
    fn the_deployer_is_the_first_link_payload() {
        let entity: Entity = serde_json::from_value(serde_json::json!({
            "type": "profile", "pointers": ["0xabc"], "timestamp": 1, "content": []
        }))
        .unwrap();
        let files = HashMap::new();
        let chain: AuthChain = serde_json::from_value(serde_json::json!([
            {"type": "SIGNER", "payload": "0xAbC"},
            {"type": "ECDSA_SIGNED_ENTITY", "payload": "bafy", "signature": "0x00"}
        ]))
        .unwrap();
        let d = Deployment {
            entity: &entity,
            files: &files,
            auth_chain: &chain,
        };
        assert_eq!(d.deployer(), "0xAbC");
        let empty = AuthChain::new();
        assert_eq!(
            Deployment {
                auth_chain: &empty,
                ..d
            }
            .deployer(),
            ""
        );
    }
}
