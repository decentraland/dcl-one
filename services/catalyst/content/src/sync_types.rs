//! Types shared by everything on the peer-sync write path: the deployment
//! line a snapshot or pointer-changes page carries, the failed-deployment
//! record the retry worker owns, and the sync error.

use serde::{Deserialize, Serialize, de::Error as _};

pub use crate::auth::{AuthChain, AuthLink, AuthLinkType};
pub use crate::wire_types::Timestamp;

pub type EntityId = String;
pub type EntityType = String;
pub type Pointer = String;

/// Peers emit timestamps as integers but some emit `1.7e12`; both can encode a
/// whole, safely representable epoch millisecond.
pub(crate) fn timestamp_number(n: &serde_json::Number) -> Option<Timestamp> {
    const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
    let n = n.as_f64()?;
    (n >= 0.0 && n <= MAX_SAFE_INTEGER && n.fract() == 0.0).then_some(n as i64)
}

fn de_timestamp<'de, D>(d: D) -> Result<Timestamp, D::Error>
where
    D: serde::Deserializer<'de>,
{
    timestamp_number(&serde_json::Number::deserialize(d)?)
        .ok_or_else(|| D::Error::custom("invalid timestamp number"))
}

fn de_timestamp_opt<'de, D>(d: D) -> Result<Option<Timestamp>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    de_timestamp(d).map(Some)
}

fn present_signature<'de, D>(d: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::deserialize(d).map(Some)
}

fn de_auth_chain<'de, D>(d: D) -> Result<AuthChain, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct WireAuthLink {
        #[serde(rename = "type")]
        link_type: AuthLinkType,
        payload: String,
        #[serde(default, deserialize_with = "present_signature")]
        signature: Option<Option<String>>,
    }

    Vec::<WireAuthLink>::deserialize(d)?
        .into_iter()
        .map(|link| {
            let invalid = if link.link_type == AuthLinkType::SIGNER {
                link.signature
                    .as_ref()
                    .and_then(Option::as_deref)
                    .is_some_and(|s| !s.is_empty())
            } else {
                link.signature.is_none()
            };
            if invalid {
                return Err(D::Error::custom("invalid auth-chain signature field"));
            }
            Ok(AuthLink {
                link_type: link.link_type,
                payload: link.payload,
                signature: link.signature.flatten(),
            })
        })
        .collect()
}

/// One line of a snapshot file, one delta of `/pointer-changes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDeployment {
    pub entity_id: EntityId,
    pub entity_type: String,
    pub pointers: Vec<Pointer>,
    #[serde(deserialize_with = "de_auth_chain")]
    pub auth_chain: AuthChain,
    #[serde(deserialize_with = "de_timestamp")]
    pub entity_timestamp: Timestamp,
    #[serde(
        default,
        deserialize_with = "de_timestamp_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub local_timestamp: Option<Timestamp>,
}

impl SyncDeployment {
    /// Wire validation before the deployer downloads files or records failures.
    pub(crate) fn validate_shape(&self) -> Result<(), &'static str> {
        if !(crate::entity::is_ipfs_v1(&self.entity_id)
            || crate::entity::is_ipfs_v2(&self.entity_id))
            || !crate::auth::is_canonical_cid(&self.entity_id)
        {
            return Err("entityId must be a CID");
        }
        if self.pointers.is_empty() {
            return Err("pointers must contain at least one entry");
        }
        if self.auth_chain.is_empty() {
            return Err("authChain must contain at least one link");
        }
        Ok(())
    }

    /// Catch malformed links that cannot pass our verifier without spending a
    /// download. Pointer changes apply their transport limits before this check.
    pub(crate) fn validate_auth_chain(&self) -> Result<(), &'static str> {
        if !crate::auth::is_valid_auth_chain(&self.auth_chain)
            || self.auth_chain.iter().any(|link| {
                link.link_type != AuthLinkType::SIGNER
                    && link.signature.as_deref().is_none_or(str::is_empty)
            })
        {
            return Err("authChain has invalid links");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureReason {
    DeploymentError,
    NoEntity,
    /// The validator could not reach a verdict: retried with backoff, never counted against
    /// the retry cap, given up on (recorded as a refusal) after a week.
    ValidationUndecided,
    /// The validator refused the entity: recorded, never retried.
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailedDeployment {
    pub entity_type: EntityType,
    pub entity_id: EntityId,
    pub reason: FailureReason,
    pub auth_chain: AuthChain,
    pub error_description: String,
    pub failure_timestamp: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_hash: Option<String>,
    /// Retry attempts already spent on this entity. The retry worker gives up once it reaches
    /// the configured cap; a plain failure report leaves it at 0 and lets the stored value stand.
    #[serde(default)]
    pub retry_count: u32,
    /// Epoch ms before which the retry worker must not touch this entity again. 0 means due now,
    /// which is what a fresh failure reports.
    #[serde(default)]
    pub next_retry_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentContext {
    Local,
    Synced,
    SyncedFix,
}

impl DeploymentContext {
    /// The `deployment_context` label of `total_deployments_count`.
    pub fn label(self) -> &'static str {
        match self {
            DeploymentContext::Local => "LOCAL",
            DeploymentContext::Synced => "SYNCED",
            DeploymentContext::SyncedFix => "FIX_ATTEMPT",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Deployment rejected: {0}")]
    DeploymentRejected(String),

    /// The validation could not be decided (a source unset or failing); retried, never a verdict.
    #[error("Validation undecided: {0}")]
    ValidationUndecided(String),

    #[error("Entity not found in storage: {entity_id}")]
    EntityNotFound { entity_id: String },

    #[error("No servers available")]
    NoServers,

    #[error("Sync stopped")]
    Stopped,

    /// A failed entity a newer active entity overtook on one of its pointers: with history
    /// pruning on, the retry discards it instead of storing it as a superseded row.
    #[error("Entity {entity_id} was overtaken by a newer deployment")]
    Overtaken { entity_id: String },
    /// A peer answered 404 for a content file.
    #[error("{0}")]
    NotFound(String),

    #[error("content hash mismatch for {hash} from {server}")]
    ContentHashMismatch { hash: String, server: String },

    #[error("{0}")]
    Other(String),
}

impl From<crate::http::SendError> for SyncError {
    fn from(e: crate::http::SendError) -> Self {
        match e {
            crate::http::SendError::Http(e) => Self::Http(e),
            refused @ crate::http::SendError::Refused(_) => Self::Other(refused.to_string()),
        }
    }
}

/// `resp`, or an error naming `what` when it is a redirect: the sync client
/// follows none, and a JSON read refuses it instead of parsing its body.
pub(crate) fn refuse_redirect(
    resp: reqwest::Response,
    what: &str,
) -> Result<reqwest::Response, SyncError> {
    if resp.status().is_redirection() {
        return Err(SyncError::Other(format!(
            "{what}: refusing to follow a {} redirect",
            resp.status()
        )));
    }
    Ok(resp)
}

impl SyncError {
    /// The reason a failed deployment is recorded under.
    pub fn failure_reason(&self) -> FailureReason {
        match self {
            SyncError::ValidationUndecided(_) => FailureReason::ValidationUndecided,
            SyncError::DeploymentRejected(_) => FailureReason::Rejected,
            _ => FailureReason::DeploymentError,
        }
    }
}

impl From<crate::storage::StorageError> for SyncError {
    fn from(e: crate::storage::StorageError) -> Self {
        SyncError::Storage(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_line_parses_integer_and_float_timestamps() {
        let line = r#"{"entityId":"bafy","entityType":"scene","pointers":["0,0"],"authChain":[{"type":"SIGNER","payload":"0xabc"}],"entityTimestamp":1.7e12}"#;
        let d: SyncDeployment = serde_json::from_str(line).unwrap();
        assert_eq!(d.entity_timestamp, 1_700_000_000_000);
        assert_eq!(d.local_timestamp, None);
        assert_eq!(d.auth_chain[0].link_type, AuthLinkType::SIGNER);
        let serialized = serde_json::to_value(&d).unwrap();
        assert!(serialized.get("localTimestamp").is_none());
        assert_eq!(
            serde_json::from_value::<SyncDeployment>(serialized).unwrap(),
            d
        );

        let line = r#"{"entityId":"bafy","entityType":"scene","pointers":[],"authChain":[],"entityTimestamp":5,"localTimestamp":7}"#;
        let d: SyncDeployment = serde_json::from_str(line).unwrap();
        assert_eq!((d.entity_timestamp, d.local_timestamp), (5, Some(7)));
    }

    #[test]
    fn an_unknown_auth_link_type_does_not_parse() {
        let line = r#"{"entityId":"bafy","entityType":"scene","pointers":[],"authChain":[{"type":"BOGUS","payload":"p"}],"entityTimestamp":1}"#;
        assert!(serde_json::from_str::<SyncDeployment>(line).is_err());
    }

    #[test]
    fn auth_link_signature_fields_follow_the_wire_shape() {
        let mut row = serde_json::json!({
            "entityId": crate::auth::hash_bytes_v1(b"auth-chain-test"),
            "entityType": "scene",
            "pointers": ["0,0"],
            "entityTimestamp": 1,
            "authChain": [
                {"type": "SIGNER", "payload": "owner"},
                {"type": "ECDSA_SIGNED_ENTITY", "payload": "entity"},
            ],
        });
        assert!(serde_json::from_value::<SyncDeployment>(row.clone()).is_err());

        row["authChain"][1]["signature"] = serde_json::Value::Null;
        let wire = serde_json::from_value::<SyncDeployment>(row.clone()).unwrap();
        assert!(wire.validate_shape().is_ok());
        assert!(wire.validate_auth_chain().is_err());

        row["authChain"][1]["signature"] = serde_json::json!("signature");
        let wire = serde_json::from_value::<SyncDeployment>(row.clone()).unwrap();
        assert!(wire.validate_auth_chain().is_ok());

        row["authChain"][0]["signature"] = serde_json::json!("invalid signer signature");
        assert!(serde_json::from_value::<SyncDeployment>(row).is_err());
    }

    #[test]
    fn a_failed_deployment_defaults_its_retry_state() {
        let record: FailedDeployment = serde_json::from_value(serde_json::json!({
            "entity_type": "scene",
            "entity_id": "bafy",
            "reason": "DeploymentError",
            "auth_chain": [],
            "error_description": "boom",
            "failure_timestamp": 1
        }))
        .unwrap();
        assert_eq!((record.retry_count, record.next_retry_at), (0, 0));
        assert_eq!(record.snapshot_hash, None);
    }
}
