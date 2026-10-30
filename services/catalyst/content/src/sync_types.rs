//! Types shared by everything on the peer-sync write path: the deployment
//! line a snapshot or pointer-changes page carries, the failed-deployment
//! record the retry worker owns, and the sync error.

use serde::{Deserialize, Serialize};

pub use crate::auth::{AuthChain, AuthLink, AuthLinkType};
pub use crate::wire_types::Timestamp;

pub type EntityId = String;
pub type EntityType = String;
pub type Pointer = String;

/// Peers emit timestamps as integers but some emit `1.7e12`; either parses.
fn de_timestamp<'de, D>(d: D) -> Result<Timestamp, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let n = serde_json::Number::deserialize(d)?;
    n.as_i64()
        .or_else(|| n.as_f64().map(|f| f as i64))
        .ok_or_else(|| serde::de::Error::custom("invalid timestamp number"))
}

fn de_timestamp_opt<'de, D>(d: D) -> Result<Option<Timestamp>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<serde_json::Number>::deserialize(d)?;
    match opt {
        None => Ok(None),
        Some(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("invalid timestamp number")),
    }
}

/// One line of a snapshot file, one delta of `/pointer-changes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDeployment {
    pub entity_id: EntityId,
    pub entity_type: String,
    pub pointers: Vec<Pointer>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureReason {
    DeploymentError,
    NoEntity,
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

    #[error("Entity not found in storage: {entity_id}")]
    EntityNotFound { entity_id: String },

    #[error("No servers available")]
    NoServers,

    #[error("Sync stopped")]
    Stopped,

    #[error("{0}")]
    Other(String),
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
